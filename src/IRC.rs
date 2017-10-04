use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::net::TcpStream;
use std::borrow::Cow;
use std::fmt;

use error::*;

// stuff from https://github.com/Detegr/RBot-parser
#[derive(PartialEq, Debug)]
pub enum Prefix<'a> {
  User(&'a str, &'a str, &'a str),
  Server(&'a str)
}
impl<'a> fmt::Display for Prefix<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match *self {
      Prefix::User(nick, user, host) => write!(f, "{}!{}@{}", nick, user, host),
      Prefix::Server(serverstr) => write!(f, "{}", serverstr)
    }
  }
}
#[derive(PartialEq, Debug)]
pub enum Command<'a> {
  Named(Cow<'a, str>),
  Numeric(u16)
}
impl<'a> fmt::Display for Command<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match *self {
      Command::Named(ref s) => write!(f, "{}", s),
      Command::Numeric(n) => write!(f, "{}", n)
    }
  }
}

#[derive(Debug)]
pub struct Message<'a> {
  pub server: &'a str,
  pub id: i64,
  pub prefix: Option<Prefix<'a>>,
  pub command: Command<'a>,
  pub params: Vec<&'a str>
}

impl<'a> fmt::Display for Message<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    // TODO: I don't think this ret.push_str() stuff is ideal
    let mut ret = match self.prefix {
      Some(ref prefix) => format!(":{} ", prefix),
      None => "".to_string()
    };
    ret.push_str(format!("{} ", self.command).as_ref());
    for param in self.params.iter() {
      // TODO: The output format of this is not 1:1 to the string that was parsed
      ret.push_str(format!("{} ", param).as_ref());
    }
    write!(f, "{}", ret)
  }
}

pub struct Context<'a> {
  pub sock: TcpStream,
  pub id: &'a str,

  pub host: &'a str,
  pub port: u16,
  
  pub nick: &'a str,
  pub ident: &'a str,
  pub realname: &'a str,

  pub channels: Vec<String>
}
impl<'a> Context<'a> {
  pub fn connect(&mut self) -> Result<()> {
    let s = format!("NICK {nick}\nUSER {user} 8 * : {realname}\n",
                    nick=self.nick,
                    user=self.ident,
                    realname=self.realname);
    self.sock.write_all(s.as_bytes()).chain_err(|| "TCP: write failure")?;
    return Ok(());
  }
  
  pub fn privmsg(&mut self, r : &str, m : &str) -> Result<()> {
    self.sock.write_all(format!("PRIVMSG {recvr} :{msg}\n",
                       recvr=r,
                       msg=m).as_bytes()).chain_err(|| "TCP: write failure")?;
    return Ok(());
  }

  pub fn join(&mut self, c : &str) -> Result<()> { 
    self.sock.write_all(format!("JOIN {channel}\n", channel=c).as_bytes()).chain_err(|| "TCP: write failure")?;
    self.channels.push(c.to_string());
    return Ok(());
  }

  pub fn part(&mut self, c : &str, r: &str) -> Result<()> {
    self.sock.write_all(format!("PART {channel} :{reason}\n",
                       channel=c,
                       reason=r).as_bytes()).chain_err(|| "TCP: write failure")?;
    return Ok(());
  }

  // TODO use less write() calls
  pub fn pong(&mut self, s : String) -> Result<()> {
    self.sock.write_all("PONG :".as_bytes()).chain_err(|| "TCP: write failure")?;
    self.sock.write_all(s.as_bytes()).chain_err(|| "TCP: write failure")?;
    self.sock.write_all("\n".as_bytes()).chain_err(|| "TCP: write failure")?;
    return Ok(());
  }
}

pub fn lookup<'a,'b>(id: &'a str, ctxs: &Vec<Context<'b>>) -> usize {
  for i in 0..ctxs.len() {
    if id == ctxs[i].id {
      return i;
    }
  }

  panic!("context not found\n");
}

pub fn readline(sock : & mut TcpStream) -> String {
  let mut buf = String::new();
  {
    let mut reader = BufReader::new(sock);
    reader.read_line(&mut buf);
    print!("{}", buf);
  };

  return buf;
}

// taken from https://github.com/Detegr/RBot-parser , i had to modify it slightly
// to get it to work correctly, so it is here instead of used a dependency
// TODO: file pull request/fork
// also TODO: port this to latest version of nom
pub mod parse {
  use std;
  use nom::*;
  use std::str::from_utf8;
  use nom::space;
  use nom::IResult::*;
  use std::str::FromStr;

  named!(nick_parser <&[u8], &str>, map_res!(chain!(nick: take_until!("!") ~ tag!("!"), ||{nick}), from_utf8));
  named!(user_parser <&[u8], &str>, map_res!(chain!(user: take_until!("@") ~ tag!("@"), ||{user}), from_utf8));
  named!(word_parser <&[u8], &str>, map_res!(take_until!(" "), from_utf8));
  named!(eol <&[u8], &str>, map_res!(take_until_and_consume!("\r"), from_utf8));

  #[derive(Debug)]
  pub struct ParserError {
    data: String
  }
  impl std::fmt::Display for ParserError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
      write!(fmt, "{}", self.data)
    }
  }
  impl std::error::Error for ParserError {
    fn description(&self) -> &str {
      &self.data
    }
  }
  impl<'a> From<Err<&'a [u8]>> for ParserError {
    fn from(e: Err<&'a [u8]>) -> ParserError {
      ParserError {
        data: format!("Error: {:?}", e)
      }
    }
  }

  named!(message_parser(&[u8]) -> (Option<super::Prefix>, super::Command, Vec<&str>),
         chain!(
           parsed_prefix: prefix_parser? ~
             parsed_command: command_parser ~
             parsed_params: map_res!(take_until_and_consume!(":"), from_utf8)? ~
             parsed_trailing: eol,
           || {
             let params = match parsed_params {
               Some(p) => {
                 let _: &str = p; // TODO: This looks stupid. How should this be done?
                 p.split_whitespace()
                   .chain(std::iter::repeat(parsed_trailing).take(1))
                   .collect()
               },
               None => parsed_trailing.split_whitespace().collect()
             };
             (
               parsed_prefix,
               parsed_command,
               params
             )
             })
  );

  named!(command_parser <&[u8], super::Command>,
         chain!(
           cmd: word_parser,
           || {
             match FromStr::from_str(cmd) {
               Ok(numericcmd) => super::Command::Numeric(numericcmd),
               Err(_) => super::Command::Named(cmd.into())
             }
           }
         )
  );
  named!(prefix_parser <&[u8], super::Prefix>,
         chain!(
           tag!(":") ~
             r: alt!(chain!(
               prefix: host_parser,
               || {
                 let (nick, user, host) = prefix;
                 super::Prefix::User(nick, user, host)
               }) |
                     chain!(
                       host: word_parser,
                       || {
                         super::Prefix::Server(host)
                       }
                     )
             ) ~
             space,
           || {r}));

  named!(host_parser <&[u8], (&str, &str, &str)>,
         chain!(
           nick: nick_parser ~
             user: user_parser ~
             host: word_parser ,
           ||{(nick, user, host)}
         )
  );

  pub fn parse_message<'a>(server: &'a str, id: i64, input: &'a str) -> Result<super::Message<'a>, ParserError> {
    match message_parser(input.as_bytes()) {
      Done(_, msg) => {
        let (parsed_prefix,
             parsed_command,
             params) = msg;
        Ok(super::Message {
          server: server,
          id: id,
          prefix: parsed_prefix,
          command: parsed_command,
          params: params
        })
      },
      Incomplete(i) => Err(ParserError {
        data: format!("Incomplete: {:?}", i)
      }),
      Error(e) => Err(From::from(e))
    }
  }
}