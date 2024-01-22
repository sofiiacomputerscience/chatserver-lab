use async_std::sync::RwLock;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

use crate::{
  core::{MessageServer, MAILBOX_SIZE, WORKPROOF_STRENGTH},
  messages::{
    self, ClientError, ClientId, ClientMessage, ClientPollReply, ClientReply, DelayedError,
    FullyQualifiedMessage, Sequence, ServerId,
  },
  workproof::verify_workproof,
};

#[cfg(feature = "federation")]
use crate::messages::{Outgoing, ServerMessage, ServerReply};

// this structure will contain the data you need to track in your server
// this will include things like delivered messages, clients last seen sequence number, etc.

struct MessageInfo {
  src: ClientId,
  content: String,
}

enum Stuff {
  Local { name: String, last_sequence: u128 },
  Remote { server: Option<ServerId> },
}

struct ClientInfo {
  stuff: Stuff,
  mailbox: VecDeque<MessageInfo>,
}

pub struct Server {
  id: ServerId,
  clients: RwLock<HashMap<ClientId, ClientInfo>>,
  routes: RwLock<HashMap<ServerId, Vec<ServerId>>>,
}

#[async_trait]
impl MessageServer for Server {
  const GROUP_NAME: &'static str = "Sofiia Boldeskul and Maksym Shyiko";

  fn new(id: ServerId) -> Self {
    Self {
      id: id,
      clients: RwLock::new(HashMap::new()),
      routes: RwLock::new(HashMap::new()),
    }
  }

  // note: you need to roll a Uuid, and then convert it into a ClientId
  // Uuid::new_v4() will generate such a value
  // you will most likely have to edit the Server struct as as to store information about the client
  async fn register_local_client(&self, name: String) -> ClientId {
    let user_id = ClientId(Uuid::new_v4());
    let mut clients = self.clients.write().await;
    clients.insert(
      user_id,
      ClientInfo {
        stuff: Stuff::Local {
          name,
          last_sequence: 0,
        },

        mailbox: VecDeque::new(),
      },
    );
    user_id
  }

  /*
   * implementation notes:
   * the workproof should be checked first
   * the nonce is in sequence.src and should be converted with (&sequence.src).into()
   * then, if the client is known, its last seen sequence number must be verified (and updated)
   */

  async fn handle_sequenced_message<A: Send>(
    &self,
    sequence: Sequence<A>,
  ) -> Result<A, ClientError> {
    let nonce: u128 = (&sequence.src).into();
    if !verify_workproof(nonce, sequence.workproof, WORKPROOF_STRENGTH) {
      return Err(ClientError::WorkProofError);
    }
    let mut clients = self.clients.write().await;
    if let Some(clientinfo) = clients.get_mut(&sequence.src) {
      match &mut clientinfo.stuff {
        Stuff::Local {
          name,
          last_sequence,
        } => {
          if *last_sequence >= sequence.seqid {
            return Err(ClientError::SequenceError);
          }
          *last_sequence = sequence.seqid;
        }
        _ => return Err(ClientError::UnknownClient),
      }
    } else {
      return Err(ClientError::UnknownClient);
    }
    Ok(sequence.content)
  }

  /* Here client messages are handled.
    * if the client is local,
      * if the mailbox is full, BoxFull should be returned
      * otherwise, Delivered should be returned
    * if the client is unknown, the message should be stored and Delayed must be returned
    * (federation) if the client is remote, Transfer should be returned

    It is recommended to write an function that handles a single message and use it to handle
    both ClientMessage variants.
  */

  async fn handle_client_message(&self, src: ClientId, msg: ClientMessage) -> Vec<ClientReply> {
    match msg {
      ClientMessage::Text { dest, content } => {
        vec![self.handle_single_message(src, dest, content).await]
      }
      ClientMessage::MText { dest, content } => {
        // processing the message for multiple destinators
        let mut replies = Vec::new();
        for recipient in dest {
          let reply = self
            .handle_single_message(src, recipient, content.clone())
            .await;
          replies.push(reply);
        }
        replies
      }
    }
  }

  /* for the given client, return the next message or error if available
   */

  async fn client_poll(&self, client: ClientId) -> ClientPollReply {
    // access all clients through mutable
    let mut clients = self.clients.write().await;

    // checking whether the client exist or no
    if let Some(client_info) = clients.get_mut(&client) {
      // if yes checking whether there are messages in the mail box of the client
      if let Some(message_info) = client_info.mailbox.pop_front() {
        ClientPollReply::Message {
          src: message_info.src,
          content: message_info.content,
        }
        // if there are no messages in the client box - return nothing
      } else {
        ClientPollReply::Nothing
      }
      // if the client wasn't found returning an error
    } else {
      ClientPollReply::DelayedError(DelayedError::UnknownRecipient(client))
    }
  }

  /* For announces
     * if the route is empty, return EmptyRoute
     * if not, store the route in some way
     * also store the remote clients
     * if one of these remote clients has messages waiting, return them
    For messages
     * if local, deliver them
     * if remote, forward them
  */

  #[cfg(feature = "federation")]
  async fn handle_server_message(&self, msg: ServerMessage) -> ServerReply {
    match msg {
      ServerMessage::Announce { route, clients } => {
        let mut routes = self.routes.write().await;
        let mut myclients = self.clients.write().await;

        match (route.last(), route.first()) {
          (Some(remote_server), Some(next_hop)) => {
            routes.insert(*remote_server, route.clone());
            for c in clients.into_keys() {
              let stuff = Stuff::Remote {
                server: Some(*remote_server),
              };
              match myclients.entry(c) {
                std::collections::hash_map::Entry::Occupied(mut e) => {
                  e.get_mut().stuff = stuff;
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                  e.insert(ClientInfo {
                    stuff,
                    mailbox: VecDeque::new(),
                  });
                }
              }
            }

            ServerReply::Outgoing(vec![])
          }
          _ => ServerReply::EmptyRoute,
        }
      }
      ServerMessage::Message(_) => todo!(),
    }
  }

  async fn list_users(&self) -> HashMap<ClientId, String> {
    let map = self.clients.read().await;
    map
      .iter()
      .filter_map(|(&client_id, client_info)| match &client_info.stuff {
        Stuff::Local { name, .. } => Some((client_id, name.clone())),
        _ => None,
      })
      .collect()
  }

  // return a route to the target server
  // bonus points if it is the shortest route
  #[cfg(feature = "federation")]
  async fn route_to(&self, destination: ServerId) -> Option<Vec<ServerId>> {
    let route = self.routes.read().await;
    match route.get(&destination) {
      Some(vec) => Some(vec.clone()),
      None => None,
    }
  }
}

//Implementation of function to deliver the message to the one dest
impl Server {
  // write your own methods here
  async fn handle_single_message(
    &self,
    src: ClientId,
    dest: ClientId,
    message: String,
  ) -> ClientReply {
    let mut myclients = self.clients.write().await;
    match myclients.get_mut(&dest) {
      Some(client) => match client.stuff {
        Stuff::Local { .. } | Stuff::Remote { server: None } => {
          if client.mailbox.len() >= MAILBOX_SIZE {
            ClientReply::Error(ClientError::BoxFull(dest))
          } else {
            let message_info = MessageInfo {
              src,
              content: message,
            };
            client.mailbox.push_back(message_info);
            ClientReply::Delivered
          }
        }
        Stuff::Remote {
          server: Some(destination_server),
        } => ClientReply::Transfer(
          destination_server,
          ServerMessage::Message(FullyQualifiedMessage {
            src,
            srcsrv: self.id,
            dsts: vec![(dest, destination_server)],
            content: message,
          }),
        ),
      },
      None => {
        log::error!("{dest:?} {src:?} {message}");
        myclients.insert(
          dest,
          ClientInfo {
            stuff: Stuff::Remote { server: None },
            mailbox: VecDeque::from([MessageInfo {
              src,
              content: message,
            }]),
          },
        );
        ClientReply::Delayed
      }
    }
  }
}

#[cfg(test)]
mod test {
  use crate::testing::test_message_server;

  use super::*;

  #[test]
  fn tester() {
    test_message_server::<Server>();
  }
}
