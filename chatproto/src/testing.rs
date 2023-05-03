#[cfg(feature = "federation")]
use std::collections::HashMap;

use anyhow::Context;

use crate::{client::Client, core::*, messages::*};

async fn sequence_correct<M: MessageServer>() -> Result<(), ClientError> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(sid);
  let c1 = server.register_local_client("user1".to_string()).await;
  let c2 = server.register_local_client("user2".to_string()).await;
  let mut client1 = Client::new(c1);
  let mut client2 = Client::new(c2);

  // send 1000 messages, correctly sequenced
  for i in 0..100 {
    let message = if i & 1 == 0 {
      client1.sequence(())
    } else {
      client2.sequence(())
    };
    server.handle_sequenced_message(message).await?;
  }
  Ok(())
}

async fn sequence_bad<M: MessageServer>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(sid);
  let c1 = server.register_local_client("user 1".to_string()).await;
  let mut client1 = Client::new(c1);
  let seq1 = client1.sequence(());
  let mut seq2 = client1.sequence(());
  seq2.seqid = seq1.seqid;
  server.handle_sequenced_message(seq1).await?;
  match server.handle_sequenced_message(seq2).await {
    Err(ClientError::SequenceError) => Ok(()),
    t => anyhow::bail!("expected a sequence error, got {:?}", t),
  }
}

async fn workproof_bad<M: MessageServer>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(sid);
  let c1 = server.register_local_client("user 1".to_string()).await;
  let r = server
    .handle_sequenced_message(Sequence {
      seqid: 1,
      src: c1,
      workproof: 0,
      content: (),
    })
    .await;
  match r {
    Err(ClientError::WorkProofError) => Ok(()),
    rr => anyhow::bail!("expected a workproof error, got {:?}", rr),
  }
}

async fn simple_client_test<M: MessageServer>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(sid);

  let c1 = server.register_local_client("user 1".to_string()).await;
  let c2 = server.register_local_client("user 2".to_string()).await;
  let r = server
    .handle_client_message(
      c1,
      ClientMessage::Text {
        dest: c2,
        content: "hello".into(),
      },
    )
    .await;
  assert_eq!(r, vec![ClientReply::Delivered]);
  let reply = server.client_poll(c2).await;
  let expected = ClientPollReply::Message {
    src: c1,
    content: "hello".into(),
  };
  if reply != expected {
    anyhow::bail!("Did not receive expected message, received {:?}", reply);
  }
  Ok(())
}

/// sends 100 single messages, and 100 multiple recipients messages
async fn multiple_client_messages_test<M: MessageServer>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(sid);

  let c1 = server.register_local_client("user 1".to_string()).await;
  let c2 = server.register_local_client("user 2".to_string()).await;
  let c3 = server.register_local_client("user 3".to_string()).await;
  for i in 0..100 {
    let r = server
      .handle_client_message(
        c1,
        ClientMessage::Text {
          dest: c2,
          content: i.to_string(),
        },
      )
      .await;
    if r != [ClientReply::Delivered] {
      anyhow::bail!("Could not deliver message {}, got {:?}", i, r);
    }
  }
  for i in 0..100 {
    let r = server
      .handle_client_message(
        c1,
        ClientMessage::MText {
          dest: vec![c2, c3],
          content: (i + 100).to_string(),
        },
      )
      .await;
    if r != [ClientReply::Delivered, ClientReply::Delivered] {
      anyhow::bail!("Could not deliver message {}, got {:?}", i, r);
    }
  }

  for i in 0..200 {
    let reply = server.client_poll(c2).await;
    let expected_reply = ClientPollReply::Message {
      src: c1,
      content: i.to_string(),
    };
    if reply != expected_reply {
      anyhow::bail!(
        "Did not receive expected message {}, received {:?}",
        i,
        reply
      );
    }
  }
  for i in 100..200 {
    let reply = server.client_poll(c3).await;
    let expected_reply = ClientPollReply::Message {
      src: c1,
      content: i.to_string(),
    };
    if reply != expected_reply {
      anyhow::bail!(
        "Did not receive expected message {}, received {:?}",
        i,
        reply
      );
    }
  }
  let reply = server.client_poll(c2).await;
  if reply != ClientPollReply::Nothing {
    anyhow::bail!(
      "Did not receive the expected Nothing (for client 2) reply, got {:?}",
      reply
    );
  }
  let reply = server.client_poll(c3).await;
  if reply != ClientPollReply::Nothing {
    anyhow::bail!(
      "Did not receive the expected Nothing (for client 3) reply, got {:?}",
      reply
    );
  }
  Ok(())
}

#[cfg(feature = "federation")]
async fn message_to_outer_user<M: MessageServer>() -> anyhow::Result<()> {
  let sid = ServerId::default();
  let server: M = MessageServer::new(sid);

  let c1 = server.register_local_client("user 1".to_string()).await;
  let s1 = ServerId::default();
  let s2 = ServerId::default();
  let s3 = ServerId::default();
  let euuid = ClientId::default();

  log::debug!("route: {} -> {} -> {} -> us", s1, s2, s3);

  let r = server
    .handle_server_message(ServerMessage::Announce {
      route: vec![s1, s2, s3],
      clients: HashMap::from([(euuid, "external user".into())]),
    })
    .await;
  if r != ServerReply::Outgoing(Vec::new()) {
    anyhow::bail!("Expected empty outgoing answer, got {:?}", r);
  }
  assert_eq!(r, ServerReply::Outgoing(Vec::new()));
  let r = server
    .handle_client_message(
      c1,
      ClientMessage::Text {
        dest: euuid,
        content: "Hello".to_string(),
      },
    )
    .await;
  let expected = [ClientReply::Transfer(
    s3,
    ServerMessage::Message(FullyQualifiedMessage {
      src: c1,
      srcsrv: sid,
      dsts: vec![(euuid, s1)],
      content: "Hello".to_string(),
    }),
  )];

  if r != expected {
    anyhow::bail!("Expected {:?}, got {:?}", expected, r)
  }

  Ok(())
}

async fn all_tests<M: MessageServer>(counter: &mut usize) -> anyhow::Result<()> {
  sequence_correct::<M>()
    .await
    .with_context(|| "sequence_correct")?;
  *counter = 1;
  sequence_bad::<M>().await.with_context(|| "sequence_bad")?;
  *counter = 2;
  workproof_bad::<M>()
    .await
    .with_context(|| "workproof_bad")?;
  *counter = 3;
  simple_client_test::<M>()
    .await
    .with_context(|| "simple_client_test")?;
  *counter = 4;
  multiple_client_messages_test::<M>()
    .await
    .with_context(|| "multiple_client_message_test")?;
  *counter = 5;
  #[cfg(feature = "federation")]
  {
    message_to_outer_user::<M>()
      .await
      .with_context(|| "message_to_outer_user")?;
    *counter = 6;
  }
  Ok(())
}

pub(crate) fn test_message_server<M: MessageServer>() {
  async_std::task::block_on(async {
    let mut counter = 0;
    match all_tests::<M>(&mut counter).await {
      Ok(()) => (),
      Err(rr) => panic!("counter={}, error={:?}", counter, rr),
    }
  });
}
