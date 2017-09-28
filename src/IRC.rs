use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::net::TcpStream;
use std;
use nom::*;
use nom;
use std::borrow::Cow;
use std::str::from_utf8;
use nom::space;
use nom::IResult::*;
use std::str::FromStr;
use std::fmt;

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
  pub nick: &'a str,
  pub ident: &'a str,
  pub realname: &'a str,

  pub channels: Vec<String>
}
impl<'a> Context<'a> {
  pub fn init(nick: &'a str, ident: &'a str, realname: &'a str) -> Context<'a> {
    return Context {
      nick: nick,
      ident: ident,
      realname: realname,
      channels: Vec::new()
    };
  }
  pub fn connect(&self,
                 sock : &mut TcpStream,
                 nick : &str,
                 user : &str,
                 realname : &str) {
    sock.write(format!("NICK {nick}\nUSER {user} 8 * : {realname}\n",
                       nick=self.nick,
                       user=self.ident,
                       realname=self.realname).as_bytes());
  }
  
  pub fn privmsg(&self, sock : &mut TcpStream, r : &str, m : &str) {
    sock.write(format!("PRIVMSG {recvr} :{msg}\n",
                       recvr=r,
                       msg=m).as_bytes());
  }

  pub fn join(&mut self, sock : &mut TcpStream, c : &str) { 
    sock.write(format!("JOIN {channel}\n", channel=c).as_bytes());
    self.channels.push(c.to_string());
  }

  pub fn part(&self, sock : &mut TcpStream, c : &str, r: &str) {
    sock.write(format!("PART {channel} :{reason}\n",
                       channel=c,
                       reason=r).as_bytes());
  }

  // TODO use less write() calls
  pub fn pong(&self, sock : &mut TcpStream, s : String) {
    sock.write("PONG :".as_bytes());
    sock.write(s.as_bytes());
    sock.write("\n".as_bytes());
  }
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

pub mod parse {
  use std;
  use nom::*;
  use nom;
  use std::borrow::Cow;
  use std::str::from_utf8;
  use nom::space;
  use nom::IResult::*;
  use std::str::FromStr;
  use std::fmt;

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
  impl<'a> From<nom::Err<&'a [u8]>> for ParserError {
    fn from(e: nom::Err<&'a [u8]>) -> ParserError {
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