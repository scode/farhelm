# terminal-host specification

NOTE: This is the contract for a program that is meant to almost never change. A running terminal host outlives every
update of the program that talks to it, so everything below that a client can observe (the wire protocol, the journal's
behavior, the child's process setup) is permanent once shipped. Read "Change policy" before changing anything in this
crate.

## What it is

A terminal host is one small, long-lived process that owns one pseudo-terminal (pty) and the one child process running
in it. It keeps a bounded, in-memory journal of everything the child writes, and serves it, along with input, resizes
and lifecycle control, to one client at a time over a unix socket.

It does not interpret terminal output. It never parses escape sequences, keeps no screen and answers no terminal
queries; all of that belongs to the client, which also gives the host an opaque snapshot of its own state now and then
so the journal can be trimmed. A client that crashes or restarts can therefore rebuild its view from the host's latest
snapshot plus the raw output after it, while the child keeps running undisturbed.

What it is not:

- Not a multiplexer. It serves exactly one terminal, and one client connection at a time.
- Not a supervisor. It does not restart its child, signal it on request, or track its descendants. It reports the
  child's exit and keeps serving until it is told to stop.
- Not persistent. Nothing it holds is ever written to disk; when the host ends, the journal is gone.
- Not an access-control boundary between processes of the same user. See "Security".

## Terms

- **Output stream**: every byte the host reads from the pty master, in order. A byte's **offset** is its position in the
  output stream, counting from 0 at spawn.
- **Record**: an entry in the journal. There are four kinds: output (a run of output bytes at an offset), resize (the
  terminal size changed at an offset), gap (output was trimmed) and exit (the child was reaped).
- **Resize index**: resizes are numbered from 0 in the order the host applied them. The size given at spawn is not a
  resize.
- **Position**: a pair (offset, resize index) naming a point in the journal: every output byte before the offset and
  every resize numbered below the index lie before it. A snapshot and a read both name a position.
- **Snapshot**: an opaque blob the client hands the host, together with the position it reflects. The host stores the
  latest accepted one and never looks inside it.
- **Tail**: the records after the snapshot's position that the host still retains.
- **Client tag**: opaque bytes given at spawn and returned verbatim in `Info`. The host attaches no meaning to them.

## Command line and child spec

```
<program> --socket <path>
```

`<program>` stands for whatever program runs the host; the contract is what follows it. `--socket` is the only argument,
and its path must be absolute; anything else is a usage error (exit status 2, one line on stderr). (The host changes to
`/` before it binds, so a relative path would name a different place than the caller meant.)

The command line deliberately carries nothing else. A host lives for weeks and its argv is readable by any process on
the machine, while the child's environment may hold credentials. Everything about the child arrives on the host's stdin
as the **child spec**: exactly one `Spawn` frame (kind 0x01, see "Framing"), after which stdin must reach end of file.
It is the one frame not limited by `MAX_FRAME`: its length field may be up to `MAX_SPEC`, so the host reads at most
`MAX_SPEC` + 4 bytes from stdin.

`Spawn` fields, in order:

| Field              | Encoding        | Meaning                                                                         |
| ------------------ | --------------- | ------------------------------------------------------------------------------- |
| `spec_version`     | u32             | 1. Any other value is refused.                                                  |
| `argv`             | list of bytes   | The child's argv, at least one entry; `argv[0]` (nonempty) is the program.      |
| `cwd`              | bytes           | The child's working directory, absolute.                                        |
| `env`              | list of bytes   | The child's complete environment, each entry `NAME=value` with a nonempty name. |
| `cols`, `rows`     | u16, u16        | Initial terminal size, each 1 to 10000.                                         |
| `journal_cap`      | u64             | Tail budget in bytes (see "Journal"), `MIN_JOURNAL_CAP` to `MAX_JOURNAL_CAP`.   |
| `tag`              | bytes           | The client tag, at most `MAX_TAG` bytes.                                        |
| `initial_snapshot` | option of bytes | A snapshot at position (0, 0), at most `MAX_SNAPSHOT` bytes. Absent means none. |

No entry of `argv` or `env`, and not `cwd`, contains a NUL byte. A spec that breaks any of these rules is refused at
start.

A program name containing `/` is run as given, relative to `cwd` when it is relative. Any other name is looked up in the
first `PATH` entry of `env` (an empty element meaning `cwd`), or in `/usr/bin:/bin` when `env` has no `PATH`; the first
regular file found that the host's user may execute (as the kernel's access check for the host's effective ids says, so
an execute bit for others alone does not count) is run, and a name found nowhere is refused at start. The file is
executed as it is: one the kernel cannot execute (a script without a `#!` line, say) is refused at start, with no retry
through `/bin/sh` as `execvp` does.

The child's environment is exactly `env`; the host adds and removes nothing, and its own environment never reaches the
child. The host's own environment is irrelevant to its behavior.

## Lifecycle

### Start and readiness

The host starts in two stages, so that the process that started it learns synchronously whether the host and its child
are running, and never has to reap a long-lived process:

1. The **first stage** reads and validates the child spec, then forks the second stage and waits.
2. The **second stage** calls `setsid` (it leads a new session and has no controlling terminal), installs its signal
   handlers, binds and listens on the socket, opens the pty, and starts the child (below). When the child has exec'd its
   program, the second stage tells the first stage it is ready.
3. The first stage exits 0 on readiness. On any failure it prints one line to stderr and exits 1, and nothing is left
   behind: the second stage reports its error and exits, after reaping a child that failed to start and removing the
   socket if it had bound it. A SIGTERM during the start leaves nothing behind either: before the handlers are installed
   there is no socket yet; once they are, the start checks for one at fixed points up to its last step before readiness:
   one seen at a check fails the start (a child already started is killed and reaped first), and one that arrives after
   the last check lets the start be reported and then ends the host at once (see "Ending"), removing the socket.

The socket path must not exist; the host never removes or replaces an existing file there. Its parent directory is the
caller's to create and protect.

After readiness, the second stage (the host proper) has stdin, stdout and stderr on `/dev/null` (whether or not its
starter left them open), its working directory is `/` (so it never keeps a directory busy), and it holds no file
descriptor except its listening socket, the pty master, its client connections and an internal signal pipe. Every
descriptor it inherited is closed. SIGHUP, SIGINT and SIGPIPE are ignored, and its signal mask is empty whatever its
starter's was: a mask survives fork and exec, and a blocked SIGTERM or SIGCHLD would keep it from ending or from
noticing its child's exit. SIGTERM ends it at once (see "Ending").

### The child

The child runs `argv` in `cwd` with exactly `env`, as the leader of a new session whose controlling terminal is the pty
slave, which is also its stdin, stdout and stderr. It starts with every signal at its default disposition and an empty
signal mask: ignored dispositions survive `exec` and a shell cannot undo them, so a child started with SIGINT or SIGPIPE
ignored would leave Ctrl-C and broken pipes dead in everything it runs. It inherits no other descriptor.

The pty starts with the kernel's default terminal settings for a new pty, then `VERASE` set to DEL (0x7f) and `IUTF8`
set. These match what tmux 3.7c gives a new pane, so a program sees the same terminal either way. Its window size is
`cols` x `rows` (pixel sizes 0).

### Child exit and end of output

Two events that are easy to conflate are kept apart:

- **Child exit**: the host reaps its direct child (it waits only for that one process; descendants it started are not
  its to reap). It then reads whatever output is already available, bounded by `EXIT_DRAIN_TIME` and `EXIT_DRAIN_BYTES`,
  appends an exit record with the child's exit status or terminating signal at the current offset, and goes on.
- **End of output**: the pty reports that no process has the slave open any more. Output that arrives after the exit
  record is journaled after it like any other. When the child exits, the kernel sends SIGHUP to its session's foreground
  process group, as for any terminal's shell. On Linux that is all: a descendant that survives it (one that ignores
  SIGHUP, or is in another process group) keeps the slave open and can go on writing for as long as it likes. macOS also
  revokes the terminal from the session at that point, so there descendants can no longer write to it.

A live child can also close its terminal descriptors. On Linux that alone ends the output, while the child runs. On
macOS the child's controlling terminal keeps the slave open, so the output ends only once the child has exited.

Neither event ends the host. It keeps serving the journal until it is told to stop. Process-tree cleanup is the client's
business; the host offers no signal message.

Input written when no process has the slave open is discarded (see "Input").

### Ending

Only three things end a host: a `Shutdown` message, SIGTERM (or any other signal that kills it), and the machine
stopping. It has no idle timeout. On `Shutdown` or SIGTERM it closes the pty master (the kernel then sends SIGHUP to the
child's session if anything still holds the slave), removes its socket file if the file at that path is still the socket
it bound (one that something else put there since is left alone), and exits 0 without waiting for anything: not for the
child, nor for anything it started. Pending input is discarded, and not acknowledged.

After `Shutdown` it first sends `Goodbye` to the client that asked: frames queued for the client and not started yet
(replies and stream records alike) are dropped, except a `Welcome` not written yet, which always comes first. So
`Goodbye` follows at most that `Welcome` and the rest of a frame already partly written. A client that wants the replies
to what it sent before `Shutdown` waits for them before sending it. From then on the host takes no new connection,
closes the connections still waiting for their `Hello`, and acts on nothing more the client sends. It ends as soon as
`Goodbye` is written, and after `GOODBYE_FLUSH` without it if the client does not read. A client whose connection closed
before `Goodbye` could be sent (see "Connections and version negotiation") gets none, and the host ends at once.

A host killed by a signal it cannot handle (SIGKILL) cannot remove its socket file, which stays behind and refuses
connections. The kernel still closes the pty master, so the child gets its hangup all the same.

## Journal

The journal is the latest snapshot plus the tail after it, all in memory:

- The **snapshot** is the initial snapshot from the child spec, or none, until a client's `Snapshot` is accepted.
- The **tail** holds output, resize and gap records after the snapshot's position, in the order they happened, and the
  exit record (which a later snapshot does not drop, so it can lie before the snapshot's position). Within one offset,
  records keep their order: a resize applied before more output arrived comes before that output.
- The tail's size is the sum of its output bytes plus `RECORD_COST` per resize record. It never exceeds `journal_cap`.
  When a new output or resize record would exceed it, the oldest tail records are dropped (output records partially,
  from the front) until it fits, and no more than that. Exit records are never dropped.

When anything was dropped, the tail starts with a **gap**: from the snapshot's offset to the position where retained
records resume, with the terminal size in force there (dropped resizes are folded into it). A client that reads across
the gap cannot rebuild what the dropped bytes did; that is a degraded state the client must handle.

The terminal size a reader starts from is therefore always known: a snapshot record carries it in the client's own
format, a gap states it, and output that follows neither (no snapshot was ever stored and nothing was dropped) starts at
the initial size, which `Info` reports.

Output is discarded only by trimming or by an accepted snapshot, never because a client read it. A gap therefore depends
only on how much output arrived since the latest accepted snapshot.

### Snapshots

A client sends `Snapshot` with a position and a blob. The host accepts it when all of these hold, and otherwise rejects
it and keeps the journal as it was. The first condition that fails, in this order, gives the rejection reason:

1. the blob is at most `MAX_SNAPSHOT` bytes (a larger one is rejected, not a malformed frame, up to what `MAX_FRAME`
   lets a frame hold);
2. the position is at or after the current snapshot's position (offset and resize index each);
3. the offset is at or after the start of retained output (which is the end of the output stream when no output is
   retained) and at or before the end of the output stream;
4. the position is **consistent with the resizes**: its resize index is at least the index of the first resize the tail
   still retains (or of the next resize, when none is retained) and at most the number of resizes applied; every resize
   numbered below it is at an offset at or before the position's offset; and every one numbered at or above it is at an
   offset at or after it. Several resizes at one offset (resizes with no output between them) may be split anywhere: the
   position may lie before, between or after them.

Accepting replaces the stored snapshot and drops every tail record before the position. Exit records stay.

### Reading

`Read` names the position the client has already processed. The host then sends what lies after it, and keeps streaming
new records as they happen:

- If the position is within the retained tail (its offset at or after the start of retained output and at or before the
  end, and consistent with the resizes as for a snapshot), the host sends the records after it.
- Otherwise (the client has nothing, or what it needs was trimmed or replaced by a snapshot), the host sends the stored
  snapshot record if there is one, then the gap record if there is one, then the whole tail. A client whose own state is
  older than the snapshot record rebuilds from it; one whose state is at or after the snapshot's position may skip the
  record and treat what follows as continuing from its own position.
- A read at exactly the stored snapshot's position counts as "otherwise" when a snapshot is stored. A client with
  nothing reads from (0, 0), which is the initial snapshot's position; one that already has the snapshot just receives
  it again.

An exit record is sent after the records before it, once, whether or not the client's position is past it; a client
reading from a later position still learns that the child exited.

A stream that sent a snapshot record and finds a newer snapshot accepted before it went on sends a gap from the snapshot
it sent to where the tail now starts, because what lay between is gone. So does a stream whose next records an accepted
snapshot dropped (a client that snapshots past what it has been sent): it receives a gap to where the tail now starts.

The host never blocks on a slow client. It writes as much as the socket accepts and keeps the rest in the journal; a
client that falls behind far enough to have its next records trimmed receives the gap and continues after it.

Where one `Output` ends and the next begins carries no meaning: it falls where the host's storage, a resize or the exit
happens to divide the output, not where the program's writes or the host's reads did.

`delivered_end` (in `Info`) is a conservative watermark of what clients were sent: the highest offset at which an
`Output` frame written whole to any client's socket ended. It never decreases, and it rises as soon as such a frame is
written, the asking client's own included, so a client rebuilding its state after a crash reads it with `Info` before
its first `Read`. No whole `Output` frame written to a client extends past it. Output below it may have been written to
an earlier connection (which may not have processed it), but not necessarily all of it: a read can start past output
that was never sent, and trimming can drop output nobody read. A frame cut short by a disconnect does not count, so a
client may have received part of the output at or above it.

## Protocol

### Framing

Every message in both directions, and the child spec, is one frame:

| Part     | Encoding | Meaning                                                                      |
| -------- | -------- | ---------------------------------------------------------------------------- |
| `length` | u32      | Bytes after this field: 1 to `MAX_FRAME` (to `MAX_SPEC` for the child spec). |
| `kind`   | u8       | The message kind.                                                            |
| `body`   | bytes    | The message's fields, in order, with nothing left over.                      |

All integers are big-endian. Field encodings:

- `u8`, `u16`, `u32`, `u64`, `i32`: fixed width.
- `bytes`: a u32 length, then that many bytes.
- `list of bytes`: a u32 count, then that many `bytes`.
- `option of X`: a u8 that is 0 (absent) or 1 (present, followed by X). Any other value is malformed.

The child spec's limit is `MAX_SPEC`; every other frame's is `MAX_FRAME`. A frame whose length is out of range, whose
kind is unknown for the negotiated version, whose body is too short or too long, or whose fields break a stated limit is
**malformed**, with one exception: a `Snapshot` blob over `MAX_SNAPSHOT` is rejected by the journal (reason 1), not
malformed.

### Connections and version negotiation

The host accepts every connection. A connection must send `Hello` first, within `HELLO_TIMEOUT`; the host closes one
that sends anything else first, sends a malformed frame, or stays silent, without a reply. It reads at most
`MAX_BEFORE_HELLO` bytes before the `Hello`: a `Hello` is 10 bytes in every version, so a connection that sent that much
without a whole frame sent something else first. At most `MAX_PENDING_CONNECTIONS` connections may be waiting for their
`Hello` at once; beyond that the oldest is closed. The host also closes, without replying, a connection whose peer runs
as a different user than the host (on platforms that report peer credentials).

`Hello` carries the highest protocol version the client speaks and a `take_over` flag. A `Hello` offering version 0 gets
`Error` 3 before the close; that is the only `Error` sent before `Welcome`, in every version. The host answers any
other:

- `Busy`, then closes the connection, when another client is connected and `take_over` is 0. A busy host is alive and
  well; the newcomer should retry later or not at all. A client the host has stopped watching (see below) can keep a
  host busy after it is gone; a client recovering a terminal it owns, after its own restart, uses `take_over`.
- otherwise `Welcome` with the version both will use: the lower of the client's and the host's. If another client was
  connected (and `take_over` was 1), that client is closed first: a frame partly written to it stays cut short, and the
  rest of what was queued for it, its pending input and whatever it sent that was not acted on yet are dropped. Nothing
  else tells it. (Kind 0x82 is reserved: it was once meant for a displacement notice, is never sent, and no version may
  use it.) An old client must therefore take an end of stream, or a reset, even one inside a frame, as possibly just
  that. A connection refused with `Busy` that sent more than its `Hello` may likewise see a reset rather than an end of
  stream.

Whole frames the client sent before a close the host observes are acted on, in order (up to one that draws an `Error`,
see "Host messages"), whatever the bounds under "Input" would otherwise hold back: a client may send `Hello` and
`Shutdown` and close without waiting for the replies. The host observes a close as an end of stream, a hangup or a
failed write to the connection. A connection whose peer shut down only its writing side counts as closed: what it sent
is acted on, and nothing more is sent to it, no reply and no `Goodbye`. `Input` among those frames is discarded as it is
decoded, never queued, as is the client's pending input; some of what the client sent before may have been written, so
its fate is unknown to the client. After observing a close, the host reads at most `MAX_FRAME` + 4 more bytes from the
connection, and frames beyond them are not acted on: that is more than an unenlarged socket buffer holds, and a bound on
a peer that stopped reading but keeps writing.

The host observes only a close of a connection it watches. A client left unwatched at the full input backstop (see
"Input") may never be observed closing: then the frames it sent and the host holds, a `Shutdown` among them, are not
acted on, the input it sent may still be written once the program reads again, and for as long as it stays the client,
every newcomer without `take_over` gets `Busy`.

`Hello`, `Welcome`, `Busy` and an `Error` sent before `Welcome` are exchanged before a version is agreed, so their
encodings are the ones given here in every version, forever: a client decodes them as version 1, and a later version can
add data to the start of a connection only in messages sent after `Welcome`.

From `Welcome` on, the connection is **the client**, and both sides use only messages of the negotiated version. This is
version 1.

### Client messages (version 1)

| Kind | Name       | Fields                                    | Host's response                                  |
| ---- | ---------- | ----------------------------------------- | ------------------------------------------------ |
| 0x10 | `Hello`    | `version` u32, `take_over` u8 (0 or 1)    | `Welcome` or `Busy` (see above)                  |
| 0x11 | `Info`     | none                                      | `InfoReply`                                      |
| 0x12 | `Read`     | `offset` u64, `resizes` u64               | records from that position on, then live records |
| 0x13 | `Snapshot` | `offset` u64, `resizes` u64, `blob` bytes | `SnapshotAccepted` or `SnapshotRejected`         |
| 0x14 | `Input`    | `data` bytes (1 to `MAX_INPUT` bytes)     | `InputAck` once resolved (see "Input")           |
| 0x15 | `Resize`   | `cols` u16, `rows` u16 (each 1 to 10000)  | a resize record in the stream                    |
| 0x16 | `Shutdown` | none                                      | `Goodbye`, then the host ends                    |

Records reach the client only after a `Read`, and a connection has one stream: a second `Read` is not allowed (`Error`
2), and a client that wants to start over reconnects. Records already in the socket give no sign of where a second
stream would start, and nothing needs one. `Hello` after `Welcome` is not allowed either (`Error` 2).

### Host messages (version 1)

| Kind | Name               | Fields                                                             |
| ---- | ------------------ | ------------------------------------------------------------------ |
| 0x80 | `Welcome`          | `version` u32                                                      |
| 0x81 | `Busy`             | none                                                               |
| 0x82 | (reserved)         | never sent; no version may use this kind                           |
| 0x83 | `InfoReply`        | see below                                                          |
| 0x84 | `SnapshotRecord`   | `offset` u64, `resizes` u64, `blob` bytes                          |
| 0x85 | `Output`           | `offset` u64, `data` bytes                                         |
| 0x86 | `Resized`          | `offset` u64, `index` u64, `cols` u16, `rows` u16                  |
| 0x87 | `Gap`              | `from` u64, `to` u64, `resizes` u64, `cols` u16, `rows` u16        |
| 0x88 | `Exited`           | `offset` u64, `status` (u8 0 = exit code, 1 = signal), `value` i32 |
| 0x89 | `SnapshotAccepted` | `offset` u64, `resizes` u64                                        |
| 0x8a | `SnapshotRejected` | `offset` u64, `resizes` u64, `reason` u8                           |
| 0x8b | `InputAck`         | `written` u64, `discarded` u64                                     |
| 0x8c | `Error`            | `code` u16, `message` bytes (at most 1024 bytes of UTF-8 text)     |
| 0x8d | `Goodbye`          | none                                                               |

`Output` data is never empty and never longer than `MAX_OUTPUT_CHUNK`. A `Gap`'s `to` and `resizes` are the position
where retained records resume: `resizes` is the index of the first resize the gap does not cover, and the size is the
size in force after every resize it covers.

`InfoReply` fields, in order: `host_pid` u32, `child_pid` u32, `child` (u8 0 = running, 1 = exited: from the moment the
host reaps the child, which can be before the `Exited` record is in the stream, since that waits for the last output;
once reaped, the pid may belong to another process), `exit_status` and `exit_value` (u8 and i32, as in `Exited`; 0 and 0
while running), `cols` u16, `rows` u16 (the size in force now), `initial_cols` u16, `initial_rows` u16 (the size given
at spawn), `snapshot_offset` u64, `snapshot_resizes` u64, `tail_start` u64 (offset where retained output starts: `end`
when none is retained), `end` u64 (offset of the next output byte), `resizes` u64 (resizes applied so far), `gap` u8 (1
when the tail starts with a gap), `delivered_end` u64, `tag` bytes. `Info` may be sent at any time after `Welcome`, also
while a stream is running; the reply is placed in the stream in order. Its values are the host's state when it acted on
the `Info`, so stream records that follow it can lie below its `end`: they were journaled before it and not sent yet.

`SnapshotRejected` reasons: 1 too large, 2 position before the current snapshot, 3 position outside retained output, 4
not consistent with the resizes (the numbered conditions under "Snapshots").

`Error` codes: 1 malformed frame after `Welcome`, 2 message not allowed in this state (`Hello` after `Welcome`, a second
`Read`), 3 unsupported version (the client offered 0). A read position is never an error: any position the tail cannot
continue from is answered by rebuilding (see "Reading").

An `Error` after `Welcome` ends the connection the way `Goodbye` ends the host. Frames queued for the client and not
started yet are dropped (a `Welcome` not written yet stays, and comes first), so the `Error` follows at most the rest of
a frame already partly written, and nothing follows it: no reply, no stream record, no `InputAck`. The client's pending
input is discarded without an acknowledgement, and nothing the client sent after the frame that drew the `Error` is
acted on, whether or not the host observed its close. The host closes the connection once the `Error` is written, and
after `GOODBYE_FLUSH` without it if the client does not read, so a refused client cannot keep newcomers on `Busy`. The
host itself keeps running.

### Input

`Input` data is queued and written to the pty in order. Each `Input` is **resolved** once: written whole, or discarded
(see below). `InputAck` carries two running totals for this connection: `written`, the input bytes that reached the pty,
and `discarded`, the input bytes that did not. The host sends one whenever one or more `Input`s were resolved since the
last, so one may cover several and their number stays bounded; `Input`s are resolved in order, so `written` plus
`discarded` is always the total size of the first `Input`s the client sent, and tells it which of them are settled.
Bytes of an `Input` that reached the pty before the rest of it was discarded count as written. Acknowledgements arrive
in order, and neither total ever decreases.

An `Input` of at most `ATOMIC_INPUT` bytes is offered to the pty whole, in one `write` call, so the host itself never
splits a key that encodes to several bytes (Shift+Enter's ESC CR). The pty may still take only part of it when its input
queue is nearly full (macOS's pty takes what fits and returns a short write), and the rest then follows in later writes.
How the program's reads divide the bytes is up to the program and its terminal settings; the host promises nothing about
it.

A program that stops reading its terminal must not wedge its host. `Resize`, `Snapshot`, `Info`, `Read` and `Shutdown`
take effect when they arrive, never queued behind input the pty has not taken, as long as the client stays within two
bounds:

- The host holds at most `MAX_PENDING_INPUT` of pending input, counting each pending `Input` as its unwritten bytes plus
  `INPUT_COST`. An `Input` that does not fit with what is pending waits, and everything the client sent after it waits
  behind it, until the pty has taken enough. This is a backstop against a misbehaving client; a well-behaved client
  limits its unacknowledged input to well under it.
- While more than `OUTGOING_HIGH_WATER` bytes of replies and stream records wait unsent, the host acts on none of the
  client's messages, so a client must keep reading its socket.

Frames that wait are still read from the socket, until one maximum frame's worth (`MAX_FRAME` + 4 bytes) is buffered.
Beyond that the host stops reading the client until they can be acted on. A client that gets there through the input
bound alone (by sending far more input than its acknowledgements allow) is then not watched at all, and the host may
never observe it close (see "Connections and version negotiation" for what follows from that). The host goes on reading
the pty and writing the stream throughout.

When the pty no longer has a reader (every process holding the slave is gone), pending and later input is discarded and
acknowledged as discarded: it went where typing into a finished terminal goes. The same holds when a write to the pty
fails for any reason other than its being full; whether output has ended is then still decided by reading the pty, so
output the program wrote before it let go is not lost. When the host observes the client's close, or the client is taken
over, sends `Shutdown` or is refused with `Error`, input it sent that was not yet written is discarded without an
acknowledgement: the connection is ending. A client that reconnects must not resend input it never saw acknowledged,
since some of it may have been written.

### Resizes

`Resize` sets the pty's window size (which signals the child's foreground process group with SIGWINCH, as any terminal
does) and appends a resize record at the current offset, which reaches the client in the stream. A resize to the size
already in force is still recorded.

## Resource bounds

| Constant                  | Value           | What happens at the limit                                  |
| ------------------------- | --------------- | ---------------------------------------------------------- |
| `MAX_FRAME`               | 16 MiB + 64 KiB | A longer frame is malformed.                               |
| `MAX_SPEC`                | 32 MiB + 64 KiB | A child spec frame with a longer length is refused.        |
| `MAX_SNAPSHOT`            | 16 MiB          | A larger snapshot is rejected (reason 1).                  |
| `MAX_TAG`                 | 4 KiB           | A longer tag is refused at start.                          |
| `MIN_JOURNAL_CAP`         | 64 KiB          | A smaller cap is refused at start.                         |
| `MAX_JOURNAL_CAP`         | 1 GiB           | A larger cap is refused at start.                          |
| `RECORD_COST`             | 64 bytes        | Per resize record, counted against the journal cap.        |
| `MAX_INPUT`               | 64 KiB          | A longer `Input` is malformed.                             |
| `ATOMIC_INPUT`            | 64 bytes        | Inputs up to this size are offered in one `write`.         |
| `MAX_PENDING_INPUT`       | 1 MiB           | An `Input` that does not fit waits, and all after it.      |
| `INPUT_COST`              | 64 bytes        | Per pending `Input`, counted against `MAX_PENDING_INPUT`.  |
| `OUTGOING_HIGH_WATER`     | 256 KiB         | Beyond this unsent, the client's messages wait.            |
| `MAX_OUTPUT_CHUNK`        | 64 KiB          | Output is sent in chunks of at most this size.             |
| `MAX_PENDING_CONNECTIONS` | 8               | The oldest connection still waiting for `Hello` is closed. |
| `MAX_BEFORE_HELLO`        | 64 bytes        | A connection without a whole frame by then is closed.      |
| `HELLO_TIMEOUT`           | 5 seconds       | A connection without `Hello` by then is closed.            |
| `EXIT_DRAIN_TIME`         | 200 ms          | Output still arriving after this follows the exit record.  |
| `EXIT_DRAIN_BYTES`        | 4 MiB           | Same.                                                      |
| `GOODBYE_FLUSH`           | 1000 ms         | Unwritten `Goodbye` or `Error`: the host ends or closes.   |

These are version 1's bounds (see "Change policy").

Memory is bounded by:

- the journal cap. Retained output is stored in blocks of up to `MAX_OUTPUT_CHUNK` bytes that small reads fill together,
  so its bookkeeping stays small next to the cap however the program writes; since only the first block can be partly
  trimmed and only the last partly filled, the blocks hold less than two blocks beyond the retained output, plus their
  queues' bookkeeping;
- the snapshot;
- the client's unacted frames: at most `MAX_FRAME` + 4 bytes, and up to twice that during the one iteration in which the
  host takes in the rest of a closed connection;
- `MAX_BEFORE_HELLO` per connection waiting for its `Hello`;
- the pending input: `MAX_PENDING_INPUT`, where `INPUT_COST` per `Input` stands for what holding it costs beyond its
  data (a queue entry, room for the queue to grow, and the allocator's overhead on the data: between about 64 and 96
  bytes), so however small the `Input`s, they take less than one and a half times the bound;
- the outgoing queue: at most `OUTGOING_HIGH_WATER` plus one frame of unsent bytes, plus acknowledgements. The host
  never queues an acknowledgement right behind one not started yet (it updates that one instead), so the only two in a
  row are one partly written at the front and one behind it, and they number at most two more than the other frames.
  Unsent bytes count whole frames, headers included, and every kind the queue can hold many of is at least 18 bytes long
  (an `Output` of one byte), but each frame also costs its queue entry and allocation, on the order of a hundred bytes;
  a queue of tiny frames can therefore take several times its byte count, a few MiB at most;
- the sockets' own buffers.

File descriptors are bounded by the fixed set above plus one client and the pending connections; when the host runs out
of descriptors, it stops accepting connections for a moment rather than retrying at once. No peer can keep the host from
the rest of its work: each turn of its loop accepts at most `MAX_PENDING_CONNECTIONS` connections and reads a bounded
amount from each source.

## Security

The host trusts every process of its own user, as any process of that user can already read its memory. Its socket's
directory is the boundary, and creating it with private permissions is the caller's job. The host additionally refuses
connections from other users where the platform reports peer credentials. Output is data: the host never interprets it,
so nothing a program prints can change what the host does.

Nothing secret is on the host's command line, and its own environment is not the child's. The host's own environment is
its starter's, readable by processes of the same user for as long as the host lives; keeping secrets out of it is the
starter's job.

## Change policy

- The protocol only grows. A message, field or behavior of a shipped version is never changed or removed. New messages
  and fields come with a new version number, used only when both sides negotiate it; a host never sends a version-N
  message to a client that negotiated less than N. A new value of an enumerated field (an error code, a rejection
  reason, an exit status kind, a child state) is a new field: a version-1 decoder refuses a value it does not know, so
  the value is sent only on connections that negotiated the version that adds it.
- Limits belong to a version. Every bound on what one frame may hold or one connection may send (`MAX_FRAME`,
  `MAX_INPUT`, `MAX_OUTPUT_CHUNK`, `MAX_PENDING_INPUT` and the like, and the ranges in the message tables) is the bound
  of the version that states it: a later version may raise one only for connections that negotiated that version, and
  each side applies the bounds of the version negotiated. (Raising `MAX_INPUT` for everyone, say, would make every
  running older host refuse a newer client's larger `Input` as malformed.) Bounds on what a host keeps and later sends
  to whichever client reads it (`MAX_SNAPSHOT`, `MAX_TAG`) and the host-wide timings and amounts (`HELLO_TIMEOUT`,
  `EXIT_DRAIN_TIME`, `EXIT_DRAIN_BYTES`, `GOODBYE_FLUSH`) are not per connection: a host has one snapshot, one tag and
  one clock for all of them, so these apply to every connection whatever version it negotiated, and a later version can
  change one only by also saying what an older connection gets instead. `RECORD_COST` is the journal's accounting and
  belongs to this group. Bounds that act before `Welcome` (`MAX_BEFORE_HELLO`, `MAX_PENDING_CONNECTIONS`) cannot depend
  on a version, since none is negotiated yet: a change to one applies to every client, and must still accept every
  shipped client's `Hello`.
- Every client must keep working with every host version ever shipped, because hosts outlive the programs that start
  them. Golden fixtures (the exact bytes of every message of every version) stay in the test suite for good.
- The journal's behavior and the child's process setup are part of the contract too: a newer client relies on an older
  running host behaving as this document says.
- Every change to this crate needs a written justification in its pull request saying which client need it serves and
  why it cannot be met without changing the host. Fixing a bug that breaks this specification is such a need; adding a
  convenience is not.
- The host's command line and the child spec are not cross-version surfaces: whoever starts a host starts it from a
  program file of its own build, so both ends of them always come from the same version. They are kept stable anyway,
  because changing them is still a change to this crate. Two fields are the exception: the tag and the initial snapshot
  reach the wire unchanged (in `InfoReply` and as a snapshot record), so `MAX_TAG` and `MAX_SNAPSHOT` are wire bounds
  too (see the bullet on limits).
