# Tensorlake's ssh gateway leaves a forwarded channel open after its sandbox end is gone

NOTE: This is a record of what was measured and inferred on 2026-11-06 and 2026-11-07, not documentation of shipped
behavior and not a confirmed diagnosis. It was written at the maintainer's request so that a possible bug report to
Tensorlake has the facts in one place. What is measured and what is inferred are kept apart below; the explanation of
_why_ the gateway behaves this way is inference.

## Short version

Through Tensorlake's ssh gateway, an OpenSSH client holding a remote-forwarded connection (`ssh -R`) never learns that
the sandbox side of that connection is gone. The sandbox side was torn down within half a second, but the local ssh
process and its connection to the local service were both still alive 185 seconds later, with keep-alives enabled or
disabled. Against an ordinary OpenSSH server the same setup ended in about 1.3 seconds. Without a forwarded connection
open, the gateway did end the client's connection promptly (about 88 ms). So the gateway appears to drop the close of a
`forwarded-tcpip` channel, while handling the session channel correctly.

## Why this came up

Recipe tunnels on Tensorlake hosts are one dedicated ssh connection per host that carries the recipe's remote forwards
(`ssh -R 127.0.0.1:<sandbox port>:<local host>:<local port>`), so that programs in the sandbox can reach services on the
helm's machine. The macos-tunnel-exit plan set out to make that connection end when the helm dies on macOS, where there
is no parent-death signal: the helm pipes the tunnel ssh's stdin, a fixed remote command in the sandbox waits on that
pipe (plus a 30-second heartbeat with a 120-second silence limit), and on end-of-input it terminates its own
`sshd-session` parent so that the sandbox listeners and forwarded connections go away. The requirement included the
local side too: the tunnel's ssh process exits and the local service's connection closes.

## What was measured

All gateway runs used the client's OpenSSH 9.6p1 on Linux, closed the pipe in-process (so Linux's parent-death cleanup
played no part), and used the smallest sandbox size. Run ids are test-recorder runs; their retained evidence is
temporary and expires with cleanup.

- **Session only, no forwarded connection held open** (run `013e8eab-a3fb-4fa3-b484-ce819f771b05`): closing the tunnel
  ssh's stdin made the local ssh exit with status 255 about 88 ms later. The gateway closed the client's connection.
- **A forwarded connection held open, short wait** (runs `dca6ab7d-e116-4341-994a-9a66e1cba923` and
  `119dfa4b-cdb8-4571-a28a-31739e456a55`): a client in the sandbox connected to the forwarded port and completed a
  round trip through it to a local test service. After stdin closed, the remote command and its `sshd-session` were gone
  and the sandbox listener refused connections, and the sandbox-side client saw end-of-file or a reset. The local ssh
  and the local end of the held connection had not finished when the probe gave up after 10 seconds.
- **The same, with a 185-second wait and a keep-alive control** (runs `6c056f69-f776-46c5-8b18-57c7d26a268c`, the
  production arguments with `ServerAliveInterval=30` and `ServerAliveCountMax=3`, and
  `cd0092d4-81b4-4624-8b34-98c054b394c5`, identical except `ServerAliveInterval=0`). A separate observer connection kept
  the sandbox from idling during the window, so idle suspension played no part.

  | Observation                                               | Keep-alive 30 s / 3 misses | Keep-alive off |
  | --------------------------------------------------------- | -------------------------- | -------------- |
  | Sandbox-side client saw end-of-file or reset              | 94 ms                      | 85 ms          |
  | Remote command and `sshd-session` gone, listener refused  | by 377 ms                  | by 371 ms      |
  | Local ssh process after 185 s (same process identity)     | still running              | still running  |
  | Local ssh's exit status, waited on directly               | never resolved             | never resolved |
  | Local service's end of the held connection after 185 s    | still open                 | still open     |
  | Exit or "Timeout, server not responding" in `-v` output   | none                       | none           |

  The client's verbose output, apart from authentication, is the same in both runs. It shows the forward set up, one
  forwarded connection opened, and then only the session channel freed:

  ```text
  debug1: remote forward success for: listen 127.0.0.1:0, connect 127.0.0.1:<local port>
  debug1: client_input_channel_open: ctype forwarded-tcpip rchan 3 win 2097152 max 32768
  debug1: channel 1: new forwarded-tcpip [127.0.0.1] (inactive timeout: 0)
  debug1: channel 1: connected to 127.0.0.1 port <local port>
  debug1: channel 0: free: client-session, nchannels 2
  ```

  Nothing about channel 1 follows, and the client never logs a keep-alive timeout, so the gateway kept answering
  keep-alives on that connection.

- **The control against an ordinary OpenSSH server** (run `f4ce3d72-fd05-48ff-81fd-cca772a05b81`, nextest
  `0e762fef-004d-4133-a1a6-72299bcacc83`): the plan's local tests ran the same tunnel arguments and remote command
  against a real unprivileged `sshd` on Linux. With a forwarded connection held open, closing stdin ended the local ssh
  and the held connection in about 1.3 seconds (`real_ssh_tether_eof_closes_a_held_forward`), and the silence limit,
  shortened for the test, did the same in about 4.3 seconds (`real_ssh_tether_silence_closes_a_held_forward`).

## What is inferred, not measured

- The gateway terminates ssh itself rather than passing TCP through to the sandbox. Every `*.sandbox.tensorlake.ai` name
  presents the same host key (measured 2026-11-03, see
  [`2026-11-03-tensorlake-sandbox-hosts-design.md`](2026-11-03-tensorlake-sandbox-hosts-design.md)), and the client
  above kept a live, keep-alive-answering connection after the sandbox's `sshd-session` was gone.
- When the sandbox-side ssh connection ends, the gateway sends the client a close for the session channel but not for
  open `forwarded-tcpip` channels, and it does not close the client's connection while such a channel is open. OpenSSH's
  client, by design, keeps running until every channel has closed, so it waits forever. With an ordinary server, the
  server's death drops the TCP connection, and the client exits at once.
- Not measured: whether the gateway relays a forwarded channel's close when only the sandbox-side client hangs up while
  the sandbox's `sshd-session` stays alive (the ordinary case of one tunneled connection ending), or whether the leftover
  client connection ever ends (the gateway was seen dropping connections idle for 300 to 400 seconds on 2026-11-03, but
  these connections answered keep-alives every 30 seconds).
- Related: the helm's long-lived `farhelm internal stdio` connection to a Tensorlake host already assumes the gateway
  can keep a dead peer's TCP connection open. Its remote side has a 120-second stdin silence watchdog for that reason
  (`SPEC_impl.md`, the Tensorlake connection section).

## A minimal reproduction for a report

1. `ssh -R 127.0.0.1:0:127.0.0.1:<port> <sandbox>@<sandbox>.sandbox.tensorlake.ai` with a remote command that stays up,
   and a local service listening on `<port>`.
2. In the sandbox, connect to the allocated forwarded port and keep that connection open.
3. Kill the connection's `sshd-session` process in the sandbox.
4. Locally, the ssh client never exits and the local service's connection stays open. Against a stock OpenSSH server,
   step 3 ends both at once.
