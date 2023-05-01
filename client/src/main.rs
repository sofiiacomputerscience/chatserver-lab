use async_std::channel::{Receiver, Sender};
use async_std::net::UdpSocket;
use chatproto::client::Client;
use chatproto::core::WORKPROOF_STRENGTH;
use chatproto::messages::{
  ClientId, ClientMessage, ClientPollReply, ClientQuery, ClientReply, Sequence,
};
use chatproto::netproto::{decode, encode};
use chatproto::workproof::gen_workproof;
use std::collections::HashMap;
use std::io::{BufRead, Cursor};
use std::net::{IpAddr, SocketAddr};
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opt {
  #[structopt(long)]
  /// your name
  name: String,

  #[structopt(long, default_value = "4666")]
  /// port to connect to
  port: u16,

  #[structopt(long, default_value = "127.0.0.1")]
  /// address to connect to
  host: IpAddr,
}

struct Network {
  socket: UdpSocket,
}

impl Network {
  async fn new(target: SocketAddr) -> anyhow::Result<Self> {
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    socket.connect(target).await?;
    Ok(Self { socket })
  }

  async fn send(&self, sq: &Sequence<ClientQuery>) -> anyhow::Result<()> {
    let mut wr = Cursor::new(Vec::new());
    encode::sequence(&mut wr, sq, encode::client_query)?;
    self.socket.send(&wr.into_inner()).await?;
    Ok(())
  }

  async fn get<X, F>(&self, f: F) -> anyhow::Result<X>
  where
    F: FnOnce(&mut Cursor<Vec<u8>>) -> anyhow::Result<X>,
  {
    let mut buf = vec![0u8; 8192];
    let n= self.socket.recv(&mut buf).await?;
    let mut cursor = Cursor::new(buf[..n].to_vec());
    f(&mut cursor)
  }
}

#[derive(Debug)]
enum Command {
  Quit,
  ListUsers,
  Message { target: String, message: String },
  Poll,
}

async fn handle_input(tx: Sender<Command>) -> anyhow::Result<()> {
  use std::os::unix::io::FromRawFd;
  let file = unsafe { std::fs::File::from_raw_fd(0) };
  let mut reader = std::io::BufReader::new(file);
  loop {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let command = line.trim_end();
    let cmd = if command.starts_with("/quit") {
      Command::Quit
    } else if command.starts_with("/list") {
      Command::ListUsers
    } else {
      match command.split_once(' ') {
        Some((target, message)) => Command::Message {
          target: target.to_string(),
          message: message.to_string(),
        },
        None => {
          eprintln!("Invalid command");
          continue;
        }
      }
    };
    log::debug!("send command: {:?}", cmd);
    tx.send(cmd).await?;
  }
}

async fn handle_network(
  client: Client,
  network: Network,
  rx: Receiver<Command>,
) -> anyhow::Result<()> {
  let mut client = client;
  let mut userlist: HashMap<ClientId, String> = HashMap::new();
  let mut ruserlist: HashMap<String, ClientId> = HashMap::new();

  loop {
    log::debug!("wariitng for comamnd");
    let cmd = rx.recv().await?;
    log::debug!("recv command: {:?}", cmd);
    match cmd {
      Command::Quit => break,
      Command::ListUsers => {
        let msg = client.sequence(ClientQuery::ListUsers);
        network.send(&msg).await?;
        let list = network.get(decode::userlist).await?;
        for (id, nm) in list.iter() {
          println!("{} - {}", id, nm);
        }
        ruserlist = list.iter().map(|(c, s)| (s.clone(), *c)).collect();
        userlist = list;
      }
      Command::Poll => {
        let msg = client.sequence(ClientQuery::Poll);
        network.send(&msg).await?;
        let reply = network.get(decode::client_poll_reply).await?;
        match reply {
          ClientPollReply::Nothing => (),
          ClientPollReply::DelayedError(msg) => eprintln!("Error: {:?}", msg),
          ClientPollReply::Message { src, content } => {
            let nm = userlist
              .get(&src)
              .cloned()
              .unwrap_or_else(|| src.to_string());
            println!("{} - {}", nm, content);
          }
        }
      }
      Command::Message { target, message } => {
        let dest = match ruserlist.get(&target) {
          None => {
            eprintln!("Unknown user {}, try polling", target);
            continue;
          }
          Some(n) => *n,
        };
        let msg = client.sequence(ClientQuery::Message(ClientMessage::Text {
          dest,
          content: message,
        }));
        network.send(&msg).await?;
        let repls = network.get(decode::client_replies).await?;
        for repl in repls {
          match repl {
            ClientReply::Delivered => (),
            ClientReply::Delayed => eprintln!("delayed ..."),
            ClientReply::Error(rr) => eprintln!("error: {}", rr),
            ClientReply::Transfer(_, _) => todo!(),
          }
        }
      }
    }
  }
  drop(rx);
  Ok(())
}

fn main() -> anyhow::Result<()> {
  async_std::task::block_on(async { main_task().await })
}

async fn main_task() -> anyhow::Result<()> {
  pretty_env_logger::init();

  let opt = Opt::from_args();
  let network = Network::new((opt.host, opt.port).into()).await?;
  let tempid = ClientId::default();
  let workproof = gen_workproof((&tempid).into(), WORKPROOF_STRENGTH, u128::MAX).unwrap();

  let sq = Sequence {
    seqid: 0,
    src: tempid,
    workproof,
    content: ClientQuery::Register(opt.name),
  };

  network.send(&sq).await?;
  let id = network.get(decode::clientid).await?;
  log::info!("registered as {}", id);
  let client = Client::new(id);

  let (tx, rx) = async_std::channel::bounded::<Command>(16);

  let itx = tx.clone();
  let tinput = async_std::task::Builder::new()
    .name("input".to_string())
    .spawn(async move { handle_input(itx).await })?;
  let tpoll = async_std::task::Builder::new()
    .name("poller".to_string())
    .spawn(async move {
      log::info!("entering main poller loop");
      loop {
        async_std::task::sleep(std::time::Duration::from_secs(1)).await;
        log::debug!("POLL");
        tx.send(Command::Poll).await.unwrap();
      }
    })?;

  handle_network(client, network, rx).await?;
  tpoll.await;
  tinput.await?;

  Ok(())
}
