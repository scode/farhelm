# A credential refusal can surface as Broken pipe

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

An agent whose session no longer exists sometimes sees "Broken pipe" instead of "credential invalid or session no longer
exists".

## Details

Paths are relative to `crates/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0349-2b597e9-145b` (audit of Area 3, agent upcall relay and
session-credential authority) as `F14 / COR-CRED-EPIPE`, tagged **possible**. Anchors and title:
`farhelm-supervisor/src/service/connection.rs:208-219`, `farhelm/src/main.rs:1353-1378`, `farhelm/src/main.rs:1503-1510`
— A credential refusal can reach the CLI as "Broken pipe"

When `farhelm agent …` or `farhelm spawn` connects to the supervisor's unix socket, both sides first exchange hello
messages, and the CLI's hello carries the session credential. The supervisor sends its own hello right away, reads the
CLI's, and then checks the credential against its store. If the check fails (stale credential, typically because the
session was deleted), it writes an `Error { req_id: 0 }` frame saying "the session credential is invalid or its session
no longer exists", then immediately `bail!`s, which drops the socket (`connection.rs:208-219`).

The CLI proceeds differently. As soon as it has read the supervisor's hello, which can arrive before the supervisor has
even begun the credential check, it writes its request. It reads a reply only if that write succeeds
(`main.rs:1503-1510` for `agent_request`, and `main.rs:1353-1378` for `spawn_session`, which has the same shape).
Usually the CLI's write lands first and the error frame is then read normally. If the CLI is descheduled long enough for
the supervisor to finish the store lookup, write the error, and close, the CLI's write fails with EPIPE. The CLI exits
with "sending the agent request: Broken pipe (os error 32)", and the real refusal sits unread in its receive buffer. On
a unix stream socket, buffered data stays readable after the peer closes.

SPEC requires a concrete error. This refusal exists precisely for the stale-credential case, and that case gets a
misleading message that depends on timing. The open premise is how often the window actually opens; it needs scheduling
delay on the CLI side. Either side can fix it:

- **Supervisor:** shut down only the write half after sending the error, and briefly drain input before dropping the
  socket.
- **CLI:** in both `agent_request` and `spawn_session`, try to read one pending frame after a failed write and report
  that frame if it is an error.
