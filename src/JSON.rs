use serde_json;

use IRC;
use ZMQ;

impl<'a> IRC::Message<'a> {
  pub fn serialize(&self) -> String {
    return json!({
      "protocol": "IRC",
      "server": self.server,
      "id": self.id,
      "prefix": match self.prefix {
        None => serde_json::value::Value::Null,
        Some(IRC::Prefix::Server(s)) => json!({
          "server": s
        }),
        Some(IRC::Prefix::User(nick, ident, host)) => json!({
          "nick": nick,
          "ident": ident,
          "host": host,
        })
      },
      "command": match self.command {
        IRC::Command::Named(ref s) => serde_json::to_value(s).unwrap(),
        IRC::Command::Numeric(n) => serde_json::to_value(n).unwrap()
      },
      "params": serde_json::to_value(&self.params).unwrap()
    }).to_string();
  }
}

pub fn okay(sock: &mut ZMQ::Socket) {
  send(sock,
       json!({
         "type": "status",
         "status": 0
       }).to_string());
}

pub fn sync(sock: &mut ZMQ::Socket,
            irc: &IRC::Context) {
  send(sock, json!({
    "protocol": "int",
    "operand" : "sync",
    "nick": irc.nick,
    "ident": irc.ident,
    "realname": irc.realname,
    "channels": serde_json::to_value(&irc.channels).unwrap()
  }).to_string());
}

fn send(sock: &mut ZMQ::Socket, s: String) {
  let msg = ZMQ::Msg::new_with_size(s.len());
  msg.data().clone_from_slice(s.as_bytes());
  msg.send(sock, 0);
}