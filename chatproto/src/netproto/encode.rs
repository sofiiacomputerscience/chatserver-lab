use std::{collections::HashMap, io::Write};

use byteorder::{LittleEndian, WriteBytesExt};
use uuid::Uuid;

use crate::messages::{
  AuthMessage, ClientId, ClientMessage, ClientPollReply, ClientQuery, ClientReply, Sequence, ServerId, ServerMessage,
};

pub fn u128<W>(w: &mut W, m: &u128) -> anyhow::Result<()>
where
  W: Write,
{
  todo!()
}

fn uuid<W>(w: &mut W, m: &Uuid) -> anyhow::Result<()>
where
  W: Write,
{
  todo!()
}

pub fn clientid<W>(w: &mut W, m: &ClientId) -> anyhow::Result<()>
where
  W: Write,
{
  todo!()
}

pub fn serverid<W>(w: &mut W, m: &ServerId) -> anyhow::Result<()>
where
  W: Write,
{
  todo!()
}

pub fn string<W>(w: &mut W, m: &str) -> anyhow::Result<()>
where
  W: Write,
{
  todo!()
}

pub fn auth<W>(w: &mut W, m: &AuthMessage) -> anyhow::Result<()>
where
  W: Write,
{
  todo!()
}

pub fn server<W>(w: &mut W, m: &ServerMessage) -> anyhow::Result<()>
where
  W: Write,
{
  todo!()
}

pub fn client<W>(w: &mut W, m: &ClientMessage) -> anyhow::Result<()>
where
  W: Write,
{
  todo!()
}

pub fn client_replies<W>(w: &mut W, m: &[ClientReply]) -> anyhow::Result<()>
where
  W: Write,
{
  todo!()
}

pub fn client_poll_reply<W>(w: &mut W, m: &ClientPollReply) -> anyhow::Result<()>
where
  W: Write,
{
  todo!()
}

pub fn userlist<W>(w: &mut W, m: &HashMap<ClientId, String>) -> anyhow::Result<()>
where
  W: Write,
{
  todo!()
}

pub fn client_query<W>(w: &mut W, m: &ClientQuery) -> anyhow::Result<()>
where
  W: Write,
{
  todo!()
}

pub fn sequence<X, W, ENC>(w: &mut W, m: &Sequence<X>, f: ENC) -> anyhow::Result<()>
where
  W: Write,
  X: serde::Serialize,
  ENC: FnOnce(&mut W, &X) -> anyhow::Result<()>,
{
  todo!()
}
