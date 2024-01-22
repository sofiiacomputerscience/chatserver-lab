use std::{any, collections::HashMap, io::Read, string, vec};

use anyhow::{anyhow, Error, Ok};
use byteorder::{LittleEndian, ReadBytesExt};
use uuid::Uuid;

use crate::{
  client,
  messages::{
    AuthMessage, ClientError, ClientId, ClientMessage, ClientPollReply, ClientQuery, ClientReply,
    DelayedError, Sequence, ServerId, ServerMessage,
  },
};

// look at the README.md for guidance on writing this function
pub fn u128<R: Read>(rd: &mut R) -> anyhow::Result<u128> {
  let val = rd.read_u8()?;
  if val < 251 {
    Ok(val as u128)
  } else {
    match val {
      251 => Ok(rd.read_u16::<LittleEndian>()? as u128),
      252 => Ok(rd.read_u32::<LittleEndian>()? as u128),
      253 => Ok(rd.read_u64::<LittleEndian>()? as u128),
      254 => Ok(rd.read_u128::<LittleEndian>()? as u128),
      _ => todo!(),
    }
  }
}

fn uuid<R: Read>(rd: &mut R) -> anyhow::Result<Uuid> {
  let len = rd.read_u8()?;
  let mut buffer = vec![0; len as usize];

  rd.read_exact(&mut buffer)?;

  // Assuming that the bytes in `buffer` represent a valid UUID
  let uuid = Uuid::from_slice(&buffer)?;

  Ok(uuid)
}

// hint: reuse uuid
pub fn clientid<R: Read>(rd: &mut R) -> anyhow::Result<ClientId> {
  let res = uuid(rd)?;
  Ok(ClientId(res))
}

// hint: reuse uuid
pub fn serverid<R: Read>(rd: &mut R) -> anyhow::Result<ServerId> {
  let res = uuid(rd)?;
  Ok(ServerId(res))
}

pub fn string<R: Read>(rd: &mut R) -> anyhow::Result<String> {
  let len = rd.read_u8()?;
  let mut buffer = vec![0; len as usize];

  rd.read_exact(&mut buffer)?;

  let res = String::from_utf8(buffer)?;

  Ok(res)
}

pub fn auth<R: Read>(rd: &mut R) -> anyhow::Result<AuthMessage> {
  let opt = u128(rd)?;
  match opt as u8 {
    0 => {
      let client = clientid(rd)?;
      let buf = rd.read_u64::<LittleEndian>()?;
      return Ok(AuthMessage::Hello {
        user: client,
        nonce: buf.to_le_bytes(),
      });
    }
    1 => {
      let server = serverid(rd)?;
      let buf = rd.read_u64::<LittleEndian>()?;
      return Ok(AuthMessage::Nonce {
        server: server,
        nonce: buf.to_le_bytes(),
      });
    }
    2 => {
      let buf: u128 = rd.read_u128::<LittleEndian>()?;
      return Ok(AuthMessage::Auth {
        response: buf.to_le_bytes(),
      });
    }
    _ => return Err(anyhow!("")),
  };
}

pub fn client<R: Read>(rd: &mut R) -> anyhow::Result<ClientMessage> {
  let opt = u128(rd)?;
  match opt as u8 {
    0 => {
      let client = clientid(rd)?;
      let content = string(rd)?;
      return Ok(ClientMessage::Text {
        dest: client,
        content: content,
      });
    }
    1 => {
      let size = u128(rd)?;
      let mut clients: Vec<ClientId> = vec![];
      for _ in 0..size {
        clients.push(clientid(rd)?);
      }
      let content = string(rd)?;
      return Ok(ClientMessage::MText {
        dest: clients,
        content: content,
      });
    }
    _ => return Err(anyhow!("")),
  };
}

pub fn client_replies<R: Read>(rd: &mut R) -> anyhow::Result<Vec<ClientReply>> {
  let size = u128(rd)? as usize; // считываем размер списка ответов
  let mut replies = Vec::with_capacity(size); // создаем вектор для хранения ответов

  for _ in 0..size {
    let reply_type = rd.read_u8()?; // считываем тип ответа
    let reply = match reply_type {
      0 => ClientReply::Delivered,
      1 => {
        let error_type = rd.read_u8()?;
        let error = match error_type {
          0 => ClientError::WorkProofError,
          1 => ClientError::UnknownClient,
          2 => ClientError::SequenceError,
          3 => {
            // для типа ошибки "BoxFull" нужно декодировать еще и ClientId
            let client_id = clientid(rd)?;
            ClientError::BoxFull(client_id)
          }
          4 => ClientError::InternalError,
          _ => return Err(anyhow!("Unknown client error type")),
        };
        ClientReply::Error(error)
      }
      2 => ClientReply::Delayed, // тип ответа "Delayed"
      3 => {
        // тип ответа "Transfer", требует декодирования ServerId и ServerMessage
        let server_id = serverid(rd)?; // декодируем ServerId
        let server_message = server(rd)?; // декодируем ServerMessage
        ClientReply::Transfer(server_id, server_message)
      }
      _ => return Err(anyhow!("Unknown client reply type")),
    };
    replies.push(reply); // добавляем декодированный ответ в список
  }

  Ok(replies) // возвращаем результат
}

pub fn client_poll_reply<R: Read>(rd: &mut R) -> anyhow::Result<ClientPollReply> {
  //  let reply_type = rd.read_u8();
  //  match reply_type { 0 => {
  //       // option message
  //       let src = clientid(rd)?;
  //       let content = string(rd)?;
  //       Ok(ClientPollReply::Message { src, content })
  //     }
  //     1 => {
  //       let error = delayed_error(rd)?;
  //       Ok(ClientPollReply::DelayedError((error)))
  //     }
  //     2 => {
  //       Ok(ClientPollReply::Nothing)
  //     }
  //   _ => Err(anyhow!("Error we don't know this type"))

  //  }
  todo!()
}

pub fn server<R: Read>(rd: &mut R) -> anyhow::Result<ServerMessage> {
  todo!()
}

pub fn userlist<R: Read>(rd: &mut R) -> anyhow::Result<HashMap<ClientId, String>> {
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
