use std::io::Read;

use bincode::Options;
use byteorder::{ReadBytesExt, LittleEndian};
use uuid::Uuid;

use crate::messages::{AuthMessage, ClientId, ClientMessage, ClientQuery, Sequence, ServerId, ServerMessage};

pub fn u128<R: Read>(rd: &mut R) -> anyhow::Result<u128> {
  todo!()
}

fn uuid<R: Read>(rd: &mut R) -> anyhow::Result<Uuid> {
  todo!()
}

pub fn clientid<R: Read>(rd: &mut R) -> anyhow::Result<ClientId> {
  todo!()
}

pub fn serverid<R: Read>(rd: &mut R) -> anyhow::Result<ServerId> {
  todo!()
}

pub fn string<R: Read>(rd: &mut R) -> anyhow::Result<String> {
  todo!()
}

pub fn auth<R: Read>(rd: &mut R) -> anyhow::Result<AuthMessage> {
  todo!()
}

pub fn client<R: Read>(rd: &mut R) -> anyhow::Result<ClientMessage> {
  todo!()
}

pub fn server<R: Read>(rd: &mut R) -> anyhow::Result<ServerMessage> {
  todo!()
}

pub fn client_query<R: Read>(rd: &mut R) -> anyhow::Result<ClientQuery> {
  todo!()
}

pub fn sequence<X, R: Read, DEC>(rd: &mut R, d: DEC) -> anyhow::Result<Sequence<X>>
where
  DEC: FnOnce(&mut R) -> anyhow::Result<X>,
{
  todo!()
}
