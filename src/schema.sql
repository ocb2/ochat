PRAGMA foreign_keys = ON;
/*
  a server has a list of users, and a list of channels
  a message is from user to user, or from user to channel,
  or from channel to user.
*/

/* taken from the ABNF in https://tools.ietf.org/html/rfc2812 */
CREATE TABLE IF NOT EXISTS messages (
  network   TEXT    NOT NULL,
  id        INTEGER NOT NULL,

  date      INTEGER NOT NULL,

  /* sent or recieved */
  direction BOOLEAN NOT NULL,

  /* prefix */
  server    TEXT,

  nick      TEXT,
  ident     TEXT,
  host      TEXT,

  /* some commands are textual, some numeric */
  command   TEXT,
  numeric   INTEGER,

  /* if we can't parse it correctly, just put the whole line in here */
  gibberish TEXT,
  PRIMARY KEY (network, id)
  /* TODO: CHECK constraints, eg if server not null then nick/ident/host should be */
);

CREATE TABLE IF NOT EXISTS params (
  /* identifier and index, respectively */
  id        INTEGER NOT NULL,
  network   TEXT    NOT NULL,
  idx       INTEGER NOT NULL,
  param     TEXT    NOT NULL,
  PRIMARY KEY (id, network, idx),
  FOREIGN KEY (id, network) REFERENCES messages(id, network)
);

/* can't be primary key because not unique */
/*CREATE INDEX date_idx ON log(date);*/