# terminal-host

A tiny process that keeps one terminal alive: it owns a pseudo-terminal and the program running in it, keeps a bounded
in-memory journal of the program's output, and serves that journal, input and resizes to one client at a time over a
unix socket. The client does all terminal interpretation; the host never parses output. A client can crash, restart or
be replaced by a newer version, and reconnect to the same host with the program still running, rebuilding its view from
the latest snapshot it handed the host plus the raw output after it.

NOTE: This is not a terminal multiplexer and has no user interface. It is a building block for a program that wants
terminals to outlive it, written so that it almost never has to change: running hosts outlive every update of their
client. [SPEC.md](SPEC.md) is the complete contract, including the change policy every change to this crate follows.

## Using it

Start a host with its socket path on the command line and the child spec (a `Spawn` frame, see SPEC.md) on stdin:

```sh
terminal-host --socket /run/user/1000/hosts/a1b2c3 < spawn.frame
```

It returns once the host and its child are running (exit 0), or with one line of error on stderr (exit 1). Then connect
to the socket and speak the protocol with the codec in [`src/protocol.rs`](src/protocol.rs), which is pure (no I/O) so
clients can use it with whatever transport they like.

A program that would rather not ship a second binary can run the same host as a subcommand of its own: the crate's entry
point, `terminal_host::main_entry`, takes the arguments that follow the subcommand and returns the exit status. (SPEC.md
writes the command line as `<program> --socket <path>` for that reason.)

## Layout

- `src/protocol.rs`: frame and message encoding and decoding, and every limit the protocol states.
- `src/journal.rs`: the journal: output, resizes, trimming, snapshots and reads, with no I/O.
- `src/host.rs`: the event loop that ties the pty, the child, the journal and the socket together.
- `src/spawn.rs`: the two-stage start, the pty and the child's process setup.
- `tests/`: real-pty integration tests, golden protocol fixtures (`tests/golden/`) and property tests.

The crate depends only on `libc` at run time, has no async runtime and no knowledge of any program that uses it.
