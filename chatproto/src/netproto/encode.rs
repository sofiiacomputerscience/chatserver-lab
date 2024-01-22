use std::{collections::HashMap, io::Write};

use byteorder::{LittleEndian, WriteBytesExt};
use uuid::Uuid;

use crate::messages::{
  AuthMessage, ClientId, ClientMessage, ClientPollReply, ClientQuery, ClientReply, Sequence,
  ServerId, ServerMessage,
};

// look at the README.md for guidance on writing this function
// this function is used to encode all the "sizes" values that will appear after that
pub fn u128<W>(w: &mut W, m: u128) -> std::io::Result<()>
where
  W: Write,
{
  if m < 251 {
    w.write_u8(m as u8)
  } else if m < u128::pow(2, 16) {
    w.write_u8(251 as u8)?;
    w.write_u16::<LittleEndian>(m as u16)
  } else if m < u128::pow(2, 32) {
    w.write_u8(252 as u8)?;
    w.write_u32::<LittleEndian>(m as u32)
  } else if m < u128::pow(2, 64) {
    w.write_u8(253 as u8)?;
    w.write_u64::<LittleEndian>(m as u64)
  } else {
    w.write_u8(254 as u8)?;
    w.write_u128::<LittleEndian>(m as u128)
  }
}

/* UUIDs are 128bit values, but in the situation they are represented as [u8; 16]
  don't forget that arrays are encoded with their sizes first, and then their content
*/
fn uuid<W>(w: &mut W, m: &Uuid) -> std::io::Result<()>
where
  W: Write,
{
  w.write_u8(m.as_bytes().len() as u8)?;
  w.write_all(m.as_bytes())
}

// reuse uuid
pub fn clientid<W>(w: &mut W, m: &ClientId) -> std::io::Result<()>
where
  W: Write,
{
  uuid(w, &m.0)
}

// reuse uuid
pub fn serverid<W>(w: &mut W, m: &ServerId) -> std::io::Result<()>
where
  W: Write,
{
  uuid(w, &m.0)
}

// strings are encoded as the underlying bytes array
// so
//  1/ get the underlying bytes
//  2/ write the size (using u128)
//  3/ write the array
pub fn string<W>(w: &mut W, m: &str) -> std::io::Result<()>
where
  W: Write,
{
  let bytes = m.as_bytes();
  let _ = w.write_u8(bytes.len() as u8);
  w.write_all(bytes)
}

/* The following is VERY mechanical, and should be easy once the general principle is understood

* Structs

   Structs are encoded by encoding all fields, by order of declaration. For example:

   struct Test {
     a: u32,
     b: [u8; 3],
   }

   Test {a: 5, b: [1, 2, 3]} -> [5, 3, 1, 2, 3]  // the first '3' is the array length
   Test {a: 42, b: [5, 6, 7]} -> [42, 3, 5, 6, 7]

* Enums

   Enums are encoded by first writing a tag, corresponding to the variant index, and the the content of the variant.
   For example, if we have:

   enum Test { A, B(u32), C(u32, u32) };

   Test::A is encoded as [0]
   Test::B(8) is encoded as [1, 8]
   Test::C(3, 17) is encoded as [2, 3, 17]

 */

pub fn auth<W>(w: &mut W, m: &AuthMessage) -> std::io::Result<()>
where
  W: Write,
{
  match m {
    AuthMessage::Hello { user, nonce } => {
      w.write_u8(0)?;
      clientid(w, user)?;
      w.write_all(nonce)
    }
    AuthMessage::Nonce { server, nonce } => {
      w.write_u8(1)?;
      serverid(w, server)?;
      w.write_all(nonce)
    }
    AuthMessage::Auth { response } => {
      w.write_u8(2)?;
      w.write_all(response)
    }
  }
}


pub fn server<W>(w: &mut W, m: &ServerMessage) -> std::io::Result<()>
where
    W: Write,
{
    match m {
        ServerMessage::Announce { route, clients } => {
            w.write_u8(0)?; // код для варианта Announce
            u128(w, route.len() as u128)?; // длина маршрута
            for server_id in route {
                serverid(w, server_id)?; // кодируем каждый ServerId в маршруте
            }
            u128(w, clients.len() as u128)?; // длина списка клиентов
            for (client_id, name) in clients {
                clientid(w, client_id)?; // кодируем ClientId
                string(w, name)?; // кодируем имя клиента (строку)
            }
            Ok(())
        }
        ServerMessage::Message(val) => {
            w.write_u8(1)?; // код для варианта Message
            clientid(w, &val.src)?; // кодируем ClientId источника
            serverid(w, &val.srcsrv)?; // кодируем ServerId источника
            u128(w, val.dsts.len() as u128)?; // длина списка адресатов
            for (dst_client_id, dst_server_id) in &val.dsts {
                clientid(w, dst_client_id)?; // кодируем ClientId назначения
                serverid(w, dst_server_id)?; // кодируем ServerId назначения
            }
            string(w, &val.content)?; // кодируем содержимое сообщения
            Ok(())
        }
    }
}

// pub fn server<W>(w: &mut W, m: &ServerMessage) -> std::io::Result<()>
// where
//   W: Write,
// {
//   match m {
//     ServerMessage::Announce { route, clients } => todo!(),
//     ServerMessage::Message(val) => {
//       // u128(w, 1)?;
//       // clientid(w, &val.src);
//       // serverid(w, &val.srcsrv);

//       todo!()
//     }
//   }
// }

pub fn client<W>(w: &mut W, m: &ClientMessage) -> std::io::Result<()>
where
  W: Write,
{
  match m {
    ClientMessage::Text { dest, content } => {
      w.write_u8(0)?;
      clientid(w, dest)?;
      string(w, content)
    }
    ClientMessage::MText { dest, content } => {
      w.write_u8(1)?;
      u128(w, dest.len() as u128)?;
      let _ = dest.iter().for_each(|x| {
        let _ = clientid(w, x);
      });
      string(w, content)
    }
  }
}

pub fn client_replies<W>(w: &mut W, m: &[ClientReply]) -> std::io::Result<()>
where
  W: Write,
{
  u128(w, m.len() as u128);
  m.iter().for_each(|x| {
    match x {
      ClientReply::Delivered => u128(w, 0),
      ClientReply::Error(val) => Ok({
        u128(w, 1);
        match val {
          crate::messages::ClientError::WorkProofError => u128(w, 0),
          crate::messages::ClientError::UnknownClient => u128(w, 1),
          crate::messages::ClientError::SequenceError => u128(w, 2),
          crate::messages::ClientError::BoxFull(x) => {
            u128(w, 3);
            clientid(w, x)
          }
          crate::messages::ClientError::InternalError => u128(w, 4),
        };
      }),
      ClientReply::Delayed => todo!(),
      ClientReply::Transfer(v1, v2) => todo!(),
    };
  });
  Ok(())
}

pub fn client_poll_reply<W>(w: &mut W, m: &ClientPollReply) -> std::io::Result<()>
where
  W: Write,
{
  todo!()
}

// hashmaps are encoded by first writing the size (using u128), then each key and values
pub fn userlist<W>(w: &mut W, m: &HashMap<ClientId, String>) -> std::io::Result<()>
where
  W: Write,
{
  todo!()
}

pub fn client_query<W>(w: &mut W, m: &ClientQuery) -> std::io::Result<()>
where
  W: Write,
{
  todo!()
}

pub fn sequence<X, W, ENC>(w: &mut W, m: &Sequence<X>, f: ENC) -> std::io::Result<()>
where
  W: Write,
  X: serde::Serialize,
  ENC: FnOnce(&mut W, &X) -> std::io::Result<()>,
{
  todo!()
}
