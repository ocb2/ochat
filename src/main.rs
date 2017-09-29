extern crate envy;
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
use std::borrow::Cow;
use std::env;
use std::io::prelude::*;
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

// FIXME: set this at runtime!
const NICK : &'static str = "nick";
const USER : &'static str = "user";
const REAL : &'static str = "real";
const CHAN : &'static str = "#channel";

//const RHOST : &'static str = "192.168.1.48";
const RHOST : &'static str = "localhost";
const RPORT : u16 = 6668;

const SCHEMA : &'static str = include_str!("schema.sql");

#[derive(Deserialize, Debug)]
struct Configuration {
  zmq_pub_listen: String,
  zmq_rep_listen: String,
  sqlite_path: String
}

fn main() {
  let config = match envy::prefixed("OCHAT_").from_env::<Configuration>() {
    Ok(config) => config,
    Err(e) => panic!("{:#?}", e)
  };
  
  let ctx = ZMQ::Context::new();
  let mut ctx_sql = Connection::open(Path::new(&config.sqlite_path)).unwrap();
  ctx_sql.execute_batch(SCHEMA).unwrap();

  let mut sock_pub = ctx.socket(ZMQ::PUB);
  let mut sock_rep = ctx.socket(ZMQ::REP);

  sock_pub.bind(&config.zmq_pub_listen);
  sock_rep.bind(&config.zmq_rep_listen);

  let mut o = TcpStream::connect((RHOST, RPORT)).unwrap();
  let mut irc = IRC::Context::init(NICK, USER, REAL);
  irc.connect(&mut o, &NICK, &USER, &REAL);
  irc.join(&mut o, &CHAN);
  o.flush();
  
  let mut items = [
    ZMQ::PollItem {
      socket: ptr::null(),
      fd: o.as_raw_fd(),
      events: ZMQ::IN | ZMQ::ERR,
      revents: 0
    },
    ZMQ::PollItem {
      socket: sock_rep.as_ptr(),
      fd: 0,
      events: ZMQ::IN | ZMQ::ERR,
      revents: 0
    }];

  let mut id : i64 = ctx_sql.query_row("SELECT MAX(id) FROM messages", &[], |r| {
    match r.get_checked::<_, i64>(0) {
      Ok(n) => n + 1,
      Err(e) => 0
    }
  }).unwrap();

  loop {
    let n = ZMQ::poll(&mut items, -1);
    println!("{:?}, {}", items, n);

    // IRC socket
    if items[0].revents > 0 {
      let now = time::now_utc().to_timespec();

      let line = IRC::readline(&mut o);
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
        Some(ref s) => if s == "PING" { irc.pong(&mut o, msg.params.get(0).unwrap().to_string()); },
        // don't send PINGs to the publisher
        _ => ()
      };

      sock_pub.send(msg.serialize().as_bytes(), 0);
    }

    // REQ/REP socket, we reply
    if items[1].revents > 0 {
      let rmsg = ZMQ::Msg::new();
      rmsg.recv(&mut sock_rep, 0);
      let c : serde_json::Value = serde_json::from_slice(rmsg.data()).unwrap();

      // operand
      match c["type"].as_str().unwrap().as_ref() {
        "SYNC" => {
          JSON::sync(&mut sock_rep, &irc);
        },
        // TODO: move this logic to the IRC module somehow...
        // also TODO: this should work with *every* IRC command
        "IRC" => {
          match c["command"].as_str().unwrap().as_ref() {
            "JOIN" => {
              irc.join(&mut o,
                       c["params"][0].as_str().unwrap().clone().as_ref());
              // when we join a new channel, broadcast new state to everyone
              JSON::sync(&mut sock_pub, &irc);
              JSON::okay(&mut sock_rep);
            },
            "PART" => {
              irc.part(&mut o,
                       c["params"][0].as_str().unwrap().clone().as_ref(),
                       c["params"][1].as_str().unwrap().clone().as_ref());
              // when we join a new channel, broadcast new state to everyone
              JSON::sync(&mut sock_pub, &irc);
              JSON::okay(&mut sock_rep);
            },
            "PRIVMSG" => {
              irc.privmsg(&mut o,
                          c["params"][0].as_str().unwrap().clone().as_ref(),
                          c["params"][1].as_str().unwrap().clone().as_ref());
              JSON::okay(&mut sock_rep);
            },
            _ => {
              JSON::sync(&mut sock_rep, &irc);
            }
          }
        }
        &_ => {}
      }
    }
  }
}