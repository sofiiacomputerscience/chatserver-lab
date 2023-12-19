use async_std::sync::RwLock;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

use crate::{
  core::{MessageServer, MAILBOX_SIZE, WORKPROOF_STRENGTH},
  messages::{
    ClientError, ClientId, ClientMessage, ClientPollReply, ClientReply, FullyQualifiedMessage,
    Sequence, ServerId,
  },
  workproof::verify_workproof,
};

#[cfg(feature = "federation")]
use crate::messages::{Outgoing, ServerMessage, ServerReply};

// this structure will contain the data you need to track in your server
// this will include things like delivered messages, clients last seen sequence number, etc.
pub struct Server {}

#[async_trait]
impl MessageServer for Server {
  const GROUP_NAME: &'static str = "TODO";

  fn new(id: ServerId) -> Self {
    todo!()
  }

  // note: you need to roll a Uuid, and then convert it into a ClientId
  // Uuid::new_v4() will generate such a value
  async fn register_local_client(&self, name: String) -> ClientId {
    todo!()
  }

  /*
   implementation notes:
   * the workproof should be checked first
    * the nonce is in sequence.src and should be converted with (&sequence.src).into()
   * then, if the client is known, its last seen sequence number must be verified (and updated)
  */
  async fn handle_sequenced_message<A: Send>(
    &self,
    sequence: Sequence<A>,
  ) -> Result<A, ClientError> {
    todo!()
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
    todo!()
  }

  /* for the given client, return the next message or error if available
   */
  async fn client_poll(&self, client: ClientId) -> ClientPollReply {
    todo!()
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

impl Server {
  // write your own methods here
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
