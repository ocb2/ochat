# ochat Client Protocol Specification (very rough draft)

This is a **very** rough draft of the client protocol for ochat. The actual program doesn't match up correctly to this yet.

The ochat protocol is defined using JSON. Each message shall contain, at a minimum, a **type** field. The three types defined so far are *sync*, *status*, and *message*. Type names are case-sensitive.

## status

The *status* type is used as a response to synchronous requests. It is structured like so:

```
{
  "type": "status",
  "status": <integer>
}
```

## sync

The *sync* type is used to synchronize the internal IRC state (nickname, channels joined to, etc) between the daemon and its clients.

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
