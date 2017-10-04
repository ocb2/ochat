# ochat Client Protocol Specification (very rough draft)

This is a **very** rough draft of the client protocol for ochat. The actual program doesn't match up correctly to this yet.

The ochat protocol is defined using JSON. Each message shall contain, at a minimum, a **type** field. The four types defined so far are *message*, *server*, *status*, and *sync*. Type names are case-sensitive.

## status

The *status* type is used as a response to synchronous requests. It is structured like so:

```
{
  "type": "status",
  "status": <integer>
}
```

## sync

The *sync* type is used to synchronize the internal IRC state (nickname, channels joined to, etc) between the daemon and its clients. It must contain a *protocol* and a *server* field.

### IRC

```
{
  "type": "sync",
  "protocol": "IRC",
  "server", <string>,
  "nick": <string>,
  "ident": <string>,
  "realname": <string>,
  "channels": [<string>, <string>, ...]
}
```

## message

The *message* type is used for protocol-specific messages, such as IRC ```PRIVMSG```, ```JOIN```, ```PART```, etc. Messages with type *message* should also contain a protocol (should sync have that too? Also, don't we need to put the server in here somewhere...). 

### IRC

```
{
  "type": "message",
  "protocol": "IRC",
  "server": <string>,
  "id": <non-negative integer>,
  "prefix": <prefix>
  "command": <string>
  "params": [<string>, <string>, ...]
}
```

A prefix looks like:

```
{
  "server": <string>
}
```

or:

```
{
  "nick": <string>,
  "ident": <string>,
  "host": <string>
}
```

or it may be ```null```.

## server

In many IRC programs, the way handling multiple servers works is that generally you put in the server details somewhere, and then it connects, and stays connected for the duration of the session, and then won't reconnect to it on the next session unless you explicitly ask it to. Since ochat is designed to run as a daemon, with no concept of sessions, it makes instead more sense to just keep connected to every server it knows about unless explicitly asked otherwise.

So, there are five primitives for manipulating the list of servers that ochat should stay connected to: ```add```, ```remove```, ```enable```, ```disable```, and ```list```. Add simply tells ochat about a server, enable tells it to stay connected to it (and then it should connect to it), disable tells it to disconnect from it entirely (and don't reconnect until an enable command is sent), and remove should make ochat forget about the server entirely (edit: should it delete logs too?). Each of the server commands must contain at least a *protocol* field, and an *id* field, and its *type* must be set to server, and contain an *operator* field which contains one of the five primitives listed earlier. All of these are to be strings, except *port* which must be a positive integer. For add, a *host* field, and a *port* field are required.

The *server* command may be initiated via the synchronous channel, or it may be broadcast to all connected clients via the publisher channel. If it is broadcast, then it should contain an additional field, *id*, an integer unique to that server. In the future, this integer should be prefixed to the message (before the JSON), so that clients can use ZMQ's subscription feature to only recieve messages for servers they wish to hear from, but that probably won't be implemented for a while.

### Return

On success, a *status* message with a status code of 0 should be sent, on failure, -1. (edit: at some point, specific failure codes should be added...)

### IRC

In IRC, several additional fields must be set: *nick*, *ident*, *real*, all strings.

#### add

```
{
  "type": "server",
  "operator": "add",
  "protocol": "IRC",
  "id": "my local server",
  "host": "localhost",
  "port": 1234,
  "nick": "hello",
  "ident": "world",
  "realname": "helloworld"
}
```

#### remove

```
{
  "type": "server",
  "operator": "remove",
  "protocol": "IRC",
  "id": "my local server"
}
```

#### enable

```
{
  "type": "server",
  "operator": "enable",
  "protocol": "IRC",
  "id": "my local server"
}
```

#### disable

```
{
  "type": "server",
  "operator": "disable",
  "protocol": "IRC",
  "id": "my local server"
}
```

#### list

```
{
  "type": "server",
  "operator": "list",
  "protocol": "IRC"
}
```

The response to this should then be:

```
[
  {
  "protocol": "IRC",
  "id": "my local server",
  "host": "localhost",
  "port": 1234,
  "nick": "hello",
  "ident": "world",
  "realname": "helloworld"
  },
  ...
]
```
