use async_std::channel::{Receiver, Sender};
use async_std::net::UdpSocket;
use async_std::sync::RwLock;
use chatproto::client::Client;
use chatproto::core::WORKPROOF_STRENGTH;
use chatproto::messages::{
  ClientId, ClientMessage, ClientPollReply, ClientQuery, ClientReply, Sequence,
};
use chatproto::netproto::{decode, encode};
use chatproto::workproof::gen_workproof;
use crossterm::event::KeyEventKind;
use crossterm::{
  event::{DisableMouseCapture, EnableMouseCapture, KeyCode},
  execute,
  terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use lazy_static::lazy_static;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;
use ratatui::{
  backend::CrosstermBackend,
  layout::{Constraint, Direction, Layout},
  style::Stylize,
  widgets::{Block, Borders},
  Terminal,
};
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::net::{IpAddr, SocketAddr};
use structopt::StructOpt;

mod inputbox;

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
    let n = self.socket.recv(&mut buf).await?;
    let mut cursor = Cursor::new(buf[..n].to_vec());
    f(&mut cursor)
  }
}

#[derive(Debug)]
enum Command {
  Quit,
  ListUsers,
  SendMessage { message: String },
  Poll,
}

enum Source {
  Me,
  Other,
}

#[derive(Default)]
struct UserInfo {
  name: String,
  active: bool,
  messages: Vec<(Source, String)>,
  unread: usize,
}

#[derive(Default)]
struct Users {
  userlist: HashMap<ClientId, UserInfo>,
  selected: Option<ClientId>,
  sorted: Vec<ClientId>,
}

lazy_static! {
  static ref USERS: RwLock<Users> = RwLock::new(Users::default());
  static ref ERRORS: RwLock<Vec<String>> = RwLock::new(Vec::new());
}

enum UIEvent {
  Key(KeyCode),
  UsersUpdated,
}

async fn handle_input(tx: Sender<UIEvent>) -> anyhow::Result<()> {
  loop {
    if let crossterm::event::Event::Key(k) = crossterm::event::read()? {
      if k.kind == KeyEventKind::Press {
        tx.send(UIEvent::Key(k.code)).await?;
        if k.code == KeyCode::Esc {
          break;
        }
      }
    }
  }
  Ok(())
}

async fn show_ui(rx: Receiver<UIEvent>, tx: Sender<Command>) -> anyhow::Result<()> {
  enable_raw_mode()?;
  let mut stdout = std::io::stdout();
  execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
  let backend = CrosstermBackend::new(stdout);
  let mut terminal = Terminal::new(backend)?;
  let mut inputbox = inputbox::IBox::new();

  loop {
    // show ui
    {
      let users = USERS.read().await;
      let errors = ERRORS.read().await;
      terminal.draw(|f| ui(f, &inputbox, &users, &errors))?;
    }

    async fn move_selected(is_up: bool) {
      let mut w = USERS.write().await;
      let ln = w.sorted.len();
      let nxt = match w
        .selected
        .as_ref()
        .and_then(|cid| w.sorted.iter().position(|p| p == cid))
      {
        None => {
          if ln == 0 {
            None
          } else {
            Some(w.sorted[if is_up { ln - 1 } else { 0 }])
          }
        }
        Some(curpos) => {
          if curpos == 0 && is_up {
            Some(w.sorted[ln - 1])
          } else {
            let npos = if is_up { curpos - 1 } else { curpos + 1 } % ln;
            Some(w.sorted[npos])
          }
        }
      };
      w.selected = nxt;
      if let Some(c) = nxt {
        if let Some(uinfo) = w.userlist.get_mut(&c) {
          uinfo.unread = 0;
        }
      }
    }

    // handle events
    let event = rx.recv().await?;
    match event {
      UIEvent::Key(k) => match k {
        KeyCode::Enter => {
          tx.send(Command::SendMessage {
            message: inputbox.message().to_string(),
          })
          .await?;
          inputbox.reset()
        }
        KeyCode::Char(to_insert) => {
          inputbox.enter_char(to_insert);
        }
        KeyCode::Backspace => {
          inputbox.delete_char();
        }
        KeyCode::Left => {
          inputbox.move_cursor_left();
        }
        KeyCode::Right => {
          inputbox.move_cursor_right();
        }
        KeyCode::Up => {
          move_selected(true).await;
        }
        KeyCode::Down => {
          move_selected(false).await;
        }
        KeyCode::Esc => {
          break;
        }
        _ => (),
      },
      UIEvent::UsersUpdated => (),
    }
  }
  disable_raw_mode()?;
  execute!(
    terminal.backend_mut(),
    LeaveAlternateScreen,
    DisableMouseCapture
  )?;
  terminal.show_cursor()?;
  tx.send(Command::Quit).await?;
  Ok(())
}

fn ui(f: &mut Frame, input: &inputbox::IBox, users: &Users, errors: &[String]) {
  let create_block = |title| {
    Block::default()
      .borders(Borders::ALL)
      .style(Style::default().fg(Color::Gray))
      .title(Span::styled(
        title,
        Style::default().add_modifier(Modifier::BOLD),
      ))
  };

  let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Percentage(70),
      Constraint::Length(3),
      Constraint::Length(10),
    ])
    .split(f.size());
  let winput = Paragraph::new(input.message())
    .style(Style::default())
    .block(create_block("Input"));
  f.render_widget(winput, chunks[1]);
  f.set_cursor(chunks[1].x + input.cursor_pos() + 1, chunks[1].y + 1);
  let errorlist = Paragraph::new(errors.iter().cloned().map(Line::from).collect::<Vec<_>>())
    .style(Style::default().fg(Color::Gray))
    .block(create_block("Errors"))
    .wrap(Wrap { trim: true });
  f.render_widget(errorlist, chunks[2]);

  let chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
    .split(chunks[0]);

  let userlist_lines = users
    .sorted
    .iter()
    .map(|cid| {
      let selected = users.selected == Some(*cid);
      let name = users
        .userlist
        .get(cid)
        .map(|u| {
          if u.unread > 0 {
            format!("{} ({})", u.name, u.unread)
          } else {
            u.name.to_string()
          }
        })
        .unwrap_or("???".to_string());
      Line::from(if selected {
        name.on_blue()
      } else {
        name.on_red()
      })
    })
    .collect::<Vec<_>>();
  let userlist = Paragraph::new(userlist_lines).block(create_block("Users"));
  f.render_widget(userlist, chunks[0]);

  let messages_lines = match users.selected.as_ref() {
    None => vec![Line::from("no user selected")],
    Some(x) => users
      .userlist
      .get(x)
      .map(|u| &u.messages)
      .iter()
      .copied()
      .flatten()
      .map(|(source, msg)| match source {
        Source::Me => Line::from(format!("> {}", msg).blue()),
        Source::Other => Line::from(format!("< {}", msg)),
      })
      .collect(),
  };
  let messages = Paragraph::new(messages_lines).block(create_block("Messages"));
  f.render_widget(messages, chunks[1]);
}

async fn handle_network(
  client: Client,
  network: Network,
  event_tx: Sender<UIEvent>,
  rx: Receiver<Command>,
) -> anyhow::Result<()> {
  let mut client = client;

  loop {
    log::debug!("waiting for command");
    let cmd = rx.recv().await?;
    log::debug!("recv command: {:?}", cmd);
    event_tx.send(UIEvent::UsersUpdated).await?;
    match cmd {
      Command::Quit => break,
      Command::ListUsers => {
        let msg = client.sequence(ClientQuery::ListUsers);
        network.send(&msg).await?;
        let list = network.get(decode::userlist).await?;
        let mut lk = USERS.write().await;
        let known_users = lk
          .userlist
          .iter()
          .map(|u| (*u.0, u.1.name.clone()))
          .collect::<HashMap<_, _>>();
        if list == known_users {
          continue;
        }
        let known_userids = known_users.keys().copied().collect::<HashSet<_>>();
        let new_userids = list.keys().copied().collect::<HashSet<_>>();
        // do not remove users that disappeared, but mark them as inactive
        for disappeared_user in known_userids.difference(&new_userids) {
          lk.userlist
            .entry(*disappeared_user)
            .and_modify(|e| e.active = false);
        }
        for new_user in new_userids.difference(&known_userids) {
          lk.userlist.insert(
            *new_user,
            UserInfo {
              active: true,
              messages: Vec::new(),
              name: list.get(new_user).unwrap().clone(),
              unread: 0,
            },
          );
        }
        let mut sref = lk.userlist.iter().collect::<Vec<_>>();
        sref.sort_by_key(|f| &f.1.name);
        lk.sorted = sref.iter().map(|f| f.0).copied().collect();
        if let Some(s) = lk.selected.as_ref() {
          if !lk.userlist.contains_key(s) {
            lk.selected = lk.userlist.keys().next().copied();
          }
        }
      }
      Command::Poll => {
        let msg = client.sequence(ClientQuery::Poll);
        network.send(&msg).await?;
        let reply = network.get(decode::client_poll_reply).await?;
        let mut lk = USERS.write().await;
        let selected = lk.selected.clone();
        match reply {
          ClientPollReply::Nothing => continue,
          ClientPollReply::DelayedError(msg) => ERRORS.write().await.push(format!("{:?}", msg)),
          ClientPollReply::Message { src, content } => {
            let uinfo = lk.userlist.entry(src).or_default();
            uinfo.messages.push((Source::Other, content));
            if selected != Some(src) {
              uinfo.unread += 1;
            }
          }
        }
      }
      Command::SendMessage { message } => {
        let mut lk = USERS.write().await;
        let target = match lk.selected.as_ref() {
          Some(t) => *t,
          None => {
            ERRORS
              .write()
              .await
              .push("Can't send message with no selected users!".to_string());
            continue;
          }
        };
        lk.userlist
          .entry(target)
          .or_default()
          .messages
          .push((Source::Me, message.clone()));
        let msg = client.sequence(ClientQuery::Message(ClientMessage::Text {
          dest: target,
          content: message,
        }));
        network.send(&msg).await?;
        let repls = network.get(decode::client_replies).await?;
        for repl in repls {
          match repl {
            ClientReply::Delivered => (),
            ClientReply::Delayed => ERRORS
              .write()
              .await
              .push(format!("message to {} delayed ...", target)),
            ClientReply::Error(rr) => ERRORS
              .write()
              .await
              .push(format!("message to {}: {}", target, rr)),
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
  let (event_tx, event_rx) = async_std::channel::bounded::<UIEvent>(32);

  let ievent_tx = event_tx.clone();
  let t_input = async_std::task::Builder::new()
    .name("input".to_string())
    .spawn(async move { handle_input(ievent_tx).await })?;

  let itx = tx.clone();
  let t_ui = async_std::task::Builder::new()
    .name("ui".to_string())
    .spawn(async move { show_ui(event_rx, itx).await })?;

  let tpoll = async_std::task::Builder::new()
    .name("poller".to_string())
    .spawn(async move {
      log::info!("entering main poller loop");
      loop {
        async_std::task::sleep(std::time::Duration::from_secs(1)).await;
        log::debug!("POLL");
        tx.send(Command::Poll).await.unwrap();
        tx.send(Command::ListUsers).await.unwrap();
      }
    })?;

  handle_network(client, network, event_tx, rx).await?;
  tpoll.await;
  t_ui.await?;
  t_input.await?;

  Ok(())
}
