extern crate envy;
#[macro_use]
extern crate error_chain;
extern crate libc;
extern crate nom;
extern crate rbot_parser;
extern crate rusqlite;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate time;

use rusqlite::Connection;
use std::io::{self, Write};
use std::net::TcpStream;
use std::os::unix::io::*;
use std::path::Path;
use std::ptr;

#[allow(non_snake_case)]
mod IRC;
#[allow(non_snake_case)]
mod JSON;
#[allow(non_snake_case)]
mod ZMQ;

const SCHEMA : &'static str = include_str!("schema.sql");

#[derive(Deserialize, Debug)]
struct Configuration {
  zmq_pub_listen: String,
  zmq_rep_listen: String,
  sqlite_path: String
}

mod error {
  error_chain! {}
}
use error::*;

// TODO: debug print statements
fn main() {
  // copy pasted from error-chain docs
  if let Err(ref e) = run() {
    println!("error: {}", e);
    
    for e in e.iter().skip(1) {
      println!("caused by: {}", e);
    }
    
    // The backtrace is not always generated. Try to run this example
    // with `RUST_BACKTRACE=1`.
    if let Some(backtrace) = e.backtrace() {
      println!("backtrace: {:?}", backtrace);
    }
    
    ::std::process::exit(1);
  } else {
    print!("Exiting.");
  }
}
// so i can use ? operator
fn run() -> Result<()> {
  // TODO: print usage on undefined environment variable
  let config = envy::prefixed("OCHAT_").from_env::<Configuration>().chain_err(|| "Environment: undefined variable")?;
  
  let ctx = ZMQ::Context::new();
  let mut ctx_sql = Connection::open(Path::new(&config.sqlite_path)).chain_err(|| "SQLite: database open failure")?;
  ctx_sql.execute_batch(SCHEMA).chain_err(|| "sqlite: schema execution failure")?;

  let mut sock_pub = ctx.socket(ZMQ::PUB);
  let mut sock_rep = ctx.socket(ZMQ::REP);

  sock_pub.bind(&config.zmq_pub_listen);
  sock_rep.bind(&config.zmq_rep_listen);

  let mut irc_ctxs = Vec::new();
  irc_ctxs.push(IRC::Context {
    sock: TcpStream::connect(("localhost", 6668)).chain_err(|| "TCP: connection failure")?,
    id: "localhost",
    host: "localhost",
    port: 6668,
    nick: "nick",
    ident: "user",
    realname: "real",
    channels: Vec::new()
  });
  irc_ctxs[0].connect();
  
  let mut items = [
    ZMQ::PollItem {
      socket: sock_rep.as_ptr(),
      fd: 0,
      events: ZMQ::IN | ZMQ::ERR,
      revents: 0
    },
    ZMQ::PollItem {
      socket: ptr::null(),
      fd: irc_ctxs[0].sock.as_raw_fd(),
      events: ZMQ::IN | ZMQ::ERR,
      revents: 0
    }];
  
  let mut id : i64 = ctx_sql.query_row("SELECT MAX(id) FROM messages", &[], |r| {
    match r.get_checked::<_, i64>(0) {
      Ok(n) => n + 1,
      Err(_) => 0
    }
  }).chain_err(|| "SQLite: query failure in id initial value lookup")?;

  loop {
    let mut ss = ZMQ::poll(&mut items, -1);

    // REQ/REP socket, we reply
    if items[0].revents > 0 {
      let rmsg = ZMQ::Msg::new();
      rmsg.recv(&mut sock_rep, 0);
      let c : serde_json::Value = serde_json::from_slice(rmsg.data()).chain_err(|| "JSON: parse failure from client")?;

      // interpret client request
      match c.get("type").and_then(|s| {
        s.as_str().and_then(|s| {
          Some(s.as_ref())})}) {
        Some("server") => {
          match c["operator"].as_str().unwrap().as_ref() {
            "add" => (),
            "edit" => (),
            "remove" => (),
            "enable" => (),
            "disable" => (),
            "list" => (),
            &_ => ()
          };
          JSON::okay(&mut sock_rep);
        },
        Some("SYNC") => {
          let i = IRC::lookup(c["id"].as_str().unwrap(), &irc_ctxs);
          let ref irc = &irc_ctxs[i];
          JSON::sync(&mut sock_rep, irc);
        },
        // TODO: move this logic to the IRC module somehow...
        // also TODO: this should work with *every* IRC command
        Some("IRC") => {
          let i = IRC::lookup(c["id"].as_str().unwrap(), &irc_ctxs);
          let ref mut irc = irc_ctxs[i];
          match c["command"].as_str().unwrap().as_ref() {
                "JOIN" => {
                  irc.join(c["params"][0].as_str().unwrap().clone().as_ref());
                  // when we join a new channel, broadcast new state to everyone
                  JSON::sync(&mut sock_pub, &irc);
                  JSON::okay(&mut sock_rep);
                },
                "PART" => {
                  irc.part(c["params"][0].as_str().unwrap().clone().as_ref(),
                           c["params"][1].as_str().unwrap().clone().as_ref());
                  // when we join a new channel, broadcast new state to everyone
                  JSON::sync(&mut sock_pub, &irc);
                  JSON::okay(&mut sock_rep);
                },
                "PRIVMSG" => {
                  irc.privmsg(c["params"][0].as_str().unwrap().clone().as_ref(),
                              c["params"][1].as_str().unwrap().clone().as_ref());
                  JSON::okay(&mut sock_rep);
                },
                _ => {
                  JSON::sync(&mut sock_rep, &irc);
                }
              }
        },
        // TODO: this should print the JSON
        // also TODO: update to rust 1.19 so i can use eprint!
        // also also TODO: make this an error-chain error?
        // also also also TODO: this whole block should be moved to its own function
        Some(_) => {io::stderr().write(b"Warning: invalid type in request").chain_err(|| "write failure")?;},
        None => {io::stderr().write(b"Warning: invalid JSON in request: missing type field").chain_err(|| "write failure")?;}
      }

      ss -= 1;
      if ss == 0 { continue; }
    }

    // IRC sockets
    for s in 1..items.len() {
      if items[s].revents > 0 {
        let ref mut irc = irc_ctxs[s-1];

        let now = time::now_utc().to_timespec();

        let line = IRC::readline(&mut irc.sock);
        let msg = IRC::parse::parse_message("localhost", id, &line).unwrap();
        //println!("items:{:?}\n, msg:{:?}\n serailize:{}\n", items, msg, msg.serialize());

        let (command, numeric) : (Option<String>, Option<u16>) = match msg.command {
          IRC::Command::Named(ref c) => (Some(c.clone().into_owned()), None),
          IRC::Command::Numeric(n) => (None, Some(n))
        };

        match msg.prefix {
          /* don't log messages without prefixes - i think this is only PING? */
          None => (),
          Some(ref p) => {
            let (server, nick, ident, host) = match p {
              &IRC::Prefix::Server(server) => (Some(server), None, None, None),
              // technically, the server can omit the user or the host, even if it gives you a nick
              &IRC::Prefix::User(nick, user, host) => (None, Some(nick), Some(user), Some(host))
            };
            // fit now into 64-bit integer
            // FIXME: doesn't this truncate the number of seconds after the year 2038?
            let then = now.sec << 32 | now.nsec as i64;
            let tx = ctx_sql.transaction().unwrap();
            // TODO: deal with gibberish
            tx.execute("INSERT INTO messages (network, id, date, server, nick, ident, host, command, numeric, gibberish) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                       &[&"localhost", &id, &then, &server, &nick, &ident, &host, &command, &numeric, &None::<&str>]).unwrap();
            let mut idx = 0;
            for ref p in msg.params.clone() {
              tx.execute("INSERT INTO params (id, network, idx, param) VALUES (?1, ?2, ?3, ?4)", &[&id, &"localhost", &idx, p]).unwrap();
              idx += 1;
            }
            tx.commit();
            id += 1;
          }
        }

        /* see if we need to respond to anything eg PING */
        match command {
          Some(ref s) => if s == "PING" { irc.pong(msg.params.get(0).unwrap().to_string()); },
          // don't send PINGs to the publisher
          _ => ()
        };

        sock_pub.send(msg.serialize().as_bytes(), 0);

        ss -= 1;
        if ss == 0 { break; }
      }
    }
  };

  return Ok(());
}