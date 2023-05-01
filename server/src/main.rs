use async_std::net::UdpSocket;
use async_std::sync::RwLock;
use async_std::task;
use chatproto::core::MessageServer;
#[cfg(feature = "federation")]
use chatproto::messages::ServerReply;
use chatproto::messages::{ClientQuery, ServerId};
use chatproto::netproto::{decode, encode};
use std::io::Cursor;
use std::net::IpAddr;
use std::sync::Arc;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opt {
  #[structopt(long, default_value = "4666")]
  /// port to listen for clients on
  cport: u16,

  #[structopt(long, default_value = "0.0.0.0")]
  /// address to listen for clients on
  clisten: IpAddr,

  #[structopt(long, default_value = "4667")]
  /// port to listen for servers on
  sport: u16,

  #[structopt(long, default_value = "0.0.0.0")]
  /// address to listen for servers on
  slisten: IpAddr,
}

#[cfg(feature = "federation")]
async fn server_thread<S: MessageServer>(listen: IpAddr, port: u16, srv: &RwLock<S>) -> std::io::Result<()> {
  let socket = UdpSocket::bind((listen, port)).await?;
  println!("Listening for servers on {}", socket.local_addr()?);
  let mut buf = vec![0u8; 8192];
  loop {
    let (n, peer) = socket.recv_from(&mut buf).await?;
    let mut cursor = Cursor::new(buf[0..n].to_vec());
    match decode::server(&mut cursor) {
      Err(rr) => eprintln!("Could not decode server message from {}: {}", peer, rr),
      Ok(msg) => match srv.write().await.handle_server_message(msg).await {
        ServerReply::Outgoing(_) => todo!(),
        ServerReply::EmptyRoute => todo!(),
        ServerReply::Error(rr) => eprintln!("Error occured when handling message from {}: {}", peer, rr),
      },
    }
  }
}

async fn client_thread<S: MessageServer>(listen: IpAddr, port: u16, srv: &RwLock<S>) -> anyhow::Result<()> {
  let socket = UdpSocket::bind((listen, port)).await?;
  println!("Listening for clients on {}", socket.local_addr()?);
  let mut buf = vec![0u8; 8192];
  loop {
    let (n, peer) = socket.recv_from(&mut buf).await?;
    let mut cursor = Cursor::new(buf[0..n].to_vec());
    match decode::sequence(&mut cursor, decode::client_query) {
      Err(rr) => eprintln!("Could not decode client message from {}: {}", peer, rr),
      Ok(m) => {
        let src = m.src;
        match srv.write().await.handle_sequenced_message(m).await {
          Ok(ClientQuery::Poll) => {
            let repl = srv.write().await.client_poll(src).await;
            let mut ocurs = Cursor::new(Vec::new());
            match encode::client_poll_reply(&mut ocurs, &repl) {
              Err(rr) => eprintln!("Could not encode {:?} for {}: {}", repl, peer, rr),
              Ok(()) => {
                if let Err(rr) = socket.send_to(&ocurs.into_inner(), peer).await {
                  eprintln!("Could not send message to peer {}: {}", peer, rr)
                }
              }
            }
          }
          Ok(ClientQuery::ListUsers) => {
            let repl = srv.write().await.list_users().await;
            let mut ocurs = Cursor::new(Vec::new());
            match encode::userlist(&mut ocurs, &repl) {
              Err(rr) => eprintln!("Could not encode {:?} for {}: {}", repl, peer, rr),
              Ok(()) => {
                if let Err(rr) = socket.send_to(&ocurs.into_inner(), peer).await {
                  eprintln!("Could not send message to peer {}: {}", peer, rr)
                }
              }
            }
          }
          Ok(ClientQuery::Register(name)) => {
            let id = srv.write().await.register_local_client(name).await;
            let mut ocurs = Cursor::new(Vec::new());
            match encode::clientid(&mut ocurs, &id) {
              Err(rr) => eprintln!("Could not encode {:?} for {}: {}", id, peer, rr),
              Ok(()) => {
                if let Err(rr) = socket.send_to(&ocurs.into_inner(), peer).await {
                  eprintln!("Could not send message to peer {}: {}", peer, rr)
                }
              }
            }
          }
          Ok(ClientQuery::Message(msg)) => {
            let repl = srv.write().await.handle_client_message(src, msg).await;
            let mut ocurs = Cursor::new(Vec::new());
            match encode::client_replies(&mut ocurs, &repl) {
              Err(rr) => eprintln!("Could not encode {:?} for {}: {}", repl, peer, rr),
              Ok(()) => {
                if let Err(rr) = socket.send_to(&ocurs.into_inner(), peer).await {
                  eprintln!("Could not send message to peer {}: {}", peer, rr)
                }
              }
            }
          }
          Err(rr) => eprintln!("Error when handling message for {}: {}", peer, rr),
        }
      }
    }
  }
}

fn main() {
  let opt = Opt::from_args();

  let server = chatproto::solutions::sample::Server::new(ServerId::default());
  let clock = Arc::new(RwLock::new(server));
  #[cfg(feature = "federation")]
  let slock = clock.clone();

  task::block_on(async move {
    let cchild = task::spawn(async move {
      if let Err(rr) = client_thread(opt.clisten, opt.cport, &clock).await {
        println!("{}", rr)
      }
    });
    #[cfg(feature = "federation")]
    let schild = task::spawn(async move {
      if let Err(rr) = server_thread(opt.slisten, opt.sport, &slock).await {
        println!("{}", rr)
      }
    });
    cchild.await;
    #[cfg(feature = "federation")]
    let _ = schild.cancel().await;
  });
}
