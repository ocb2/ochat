extern crate libc;

use std::slice;
use std::ops::BitOr;

pub enum SocketType {
  PAIR = 0,
  PUB = 1,
  SUB = 2,
  REQ = 3,
  REP = 4,
  DEALER = 5,
  ROUTER = 6,
  PULL = 7,
  PUSH = 8,
  XPUB = 9,
  XSUB = 10,
  STREAM = 11,
}

pub enum PollEvent {
  IN = 1,
  OUT = 2,
  ERR = 4,
  PRI = 8
}
impl BitOr for PollEvent {
  type Output = i16;
  fn bitor(self, rhs: PollEvent) -> i16 {
    self as i16 | rhs as i16
  }
}

pub use self::SocketType::*;
pub use self::PollEvent::*;

// set either socket or fd, if both set then fd ignored
#[repr(C)]
#[derive(Debug)]
pub struct PollItem {
  pub socket: *const libc::c_void,
  pub fd: libc::c_int,
  pub events: libc::c_short,
  pub revents: libc::c_short
}

#[repr(C)]
pub struct Msg {
  pub data: [u8; 64]
}

#[link(name = "zmq")]
extern {
  fn zmq_bind(socket: *const libc::c_void,
              endpoint: *const libc::c_char) -> libc::c_int;
  fn zmq_ctx_new() -> *const libc::c_void;
  fn zmq_errno() -> libc::c_int;
  fn zmq_msg_data(msg: *const Msg) -> *const libc::c_void;
  fn zmq_msg_init(msg: *mut Msg) -> libc::c_int;
  fn zmq_msg_init_data(msg: *mut Msg,
                       data: *const libc::c_void,
                       size: libc::size_t,
                       ffn: extern fn(data: *const libc::c_void,
                                      hint: *const libc::c_void),
                       hint: *const libc::c_void) -> libc::c_int;
  fn zmq_msg_init_size(msg: *mut Msg,
                       size: libc::size_t) -> libc::c_int;
  fn zmq_msg_recv(msg: *const Msg,
                  socket: *const libc::c_void,
                  flags: libc::c_int) -> libc::c_int;
  fn zmq_msg_size(msg: *const Msg) -> libc::c_int;
  fn zmq_msg_send(msg: *const Msg,
                  socket: *const libc::c_void,
                  flags: libc::c_int) -> libc::c_int;
  fn zmq_poll(items: *mut PollItem,
              nitems: libc::c_int,
              timeout: libc::c_long) -> libc::c_int;
  fn zmq_send(socket: *const libc::c_void,
              buf: *const libc::c_void,
              len: libc::size_t,
              flags: libc::c_int) -> libc::c_int;
  fn zmq_socket(context: *const libc::c_void,
                type_: libc::c_int) -> *const libc::c_void;
}

pub struct Context { ptr: *const libc::c_void }
impl Context {
  pub fn new() -> Context {
    return Context { ptr: unsafe { zmq_ctx_new() } };
  }

  pub fn socket(&self, t : SocketType) -> Socket {
    return Socket { ptr: unsafe { zmq_socket(self.ptr, t as i32) } };
  }
}

pub struct Socket { ptr: *const libc::c_void }
impl Socket {
  pub fn as_ptr(&self) -> *const libc::c_void {
    return self.ptr;
  }
  pub fn bind(&self, endpoint: &str) -> i32 {
    unsafe { zmq_bind(self.ptr, endpoint.as_ptr() as *const i8) }
  }
  pub fn send(&self, buf: &[u8], flags: i32) -> i32 {
    unsafe { zmq_send(self.ptr, buf.as_ptr() as *const libc::c_void, buf.len(), flags) }
  }
}

impl Msg {
  pub fn new() -> Msg {
    let mut msg = Msg { data: [0; 64] };
    unsafe { zmq_msg_init(&mut msg) };
    return msg;
  }
//  pub fn new_from_data(&self, data: &[u8], callback: fn(&[u8], )) {}
  pub fn new_with_size(size: usize) -> Msg {
    let mut msg = Msg { data: [0; 64] };
    unsafe { zmq_msg_init_size(&mut msg, size) };
    return msg;
  }
  pub fn data(&self) -> &mut [u8] {
    return unsafe { slice::from_raw_parts_mut(zmq_msg_data(self) as *mut u8,
                                              self.size() as usize) };
  }
  pub fn recv(&self, socket: &mut Socket, flags: i32) -> i32 {
    return unsafe { zmq_msg_recv(self, socket.ptr, flags) };
  }
  pub fn send(&self, socket: &mut Socket, flags: i32) -> i32 {
    return unsafe { zmq_msg_send(self, socket.ptr, flags) };
  }
  pub fn size(&self) -> i32 {
    return unsafe { zmq_msg_size(self) };
  }
}

pub fn errno() -> i32 {
  return unsafe { zmq_errno() };
}

// note: rolls over if items has >2^32 elements
pub fn poll(items: &mut [PollItem], timeout: i64) -> i32 {
  return unsafe { zmq_poll(items.as_ptr() as *mut _, items.len() as i32, timeout) };
}