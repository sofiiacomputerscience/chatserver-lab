use async_std::sync::RwLock;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

use crate::{
  core::{MessageServer, MAILBOX_SIZE, WORKPROOF_STRENGTH},
  messages::{
    self, ClientError, ClientId, ClientMessage, ClientPollReply, ClientReply,
    FullyQualifiedMessage, Sequence, ServerId, DelayedError,
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

struct ClientInfo {
  name: String,
  last_sequence: u128,
  mailbox: VecDeque<MessageInfo>,
}
pub struct Server {
  id: ServerId,
  clients: RwLock<HashMap<ClientId, ClientInfo>>,
}

#[async_trait]
impl MessageServer for Server {
  const GROUP_NAME: &'static str = "Sofiia Boldeskul and Maksym Shyiko";

  fn new(id: ServerId) -> Self {
    Self {
      id: id,
      clients: RwLock::new(HashMap::new()),
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
        name,
        last_sequence: 0,
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
    if let Some(client_data) = clients.get_mut(&sequence.src) {
      if client_data.last_sequence >= sequence.seqid {
        return Err(ClientError::SequenceError);
      }
      client_data.last_sequence = sequence.seqid;
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
        // processing the message for multiole destinators
        let mut replies = Vec::new();
        for recipient in dest {
            let reply = self.handle_single_message(src, recipient, content.clone()).await;
            replies.push(reply);
        }
        replies
      }
    } 
  }

  /* for the given client, return the next message or error if available
   */
  async fn client_poll(&self, client: ClientId) -> ClientPollReply {
    let mut clients = self.clients.write().await;

    if let Some(client_info) = clients.get_mut(&client){
      if let Some(message_info) = client_info.mailbox.pop_front(){
        ClientPollReply::Message { src: message_info.src, content: message_info.content }

      }
      else {
        ClientPollReply::Nothing
      }
    }
    else {
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
    todo!()
  }

  async fn list_users(&self) -> HashMap<ClientId, String> {
    todo!()
  }

  // return a route to the target server
  // bonus points if it is the shortest route
  #[cfg(feature = "federation")]
  async fn route_to(&self, destination: ServerId) -> Option<Vec<ServerId>> {
    todo!()
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
    match self.clients.write().await.get_mut(&dest) {
      Some(client) => {
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
      None => ClientReply::Delayed,
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
