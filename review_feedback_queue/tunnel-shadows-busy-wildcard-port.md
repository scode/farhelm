# A Mac tunnel can take a port a local service already serves on all addresses

Reviewed commit: 08ab6e08f9ddd40b5494f2cbbeaaeb9d4e3332b7

## TLDR

When you open a localhost tunnel to a remote session's service, the Mac app tries to use the same port number on the
Mac. If a program of yours on the Mac already serves that port on all addresses (a container Docker publishes, or a dev
server with its default bind), the tunnel can apparently still take that port on `127.0.0.1` instead of falling back to
a free one. Mac programs that connect to `127.0.0.1` on that port then talk to the remote host instead of your local
service, sending it what they would send the local one: database passwords, API tokens, writes meant for the local
database. The remote agent picks the port by printing the link, so a remote host chooses which local service gets
shadowed. The consent warning promises the remote port is used only "when free", and here it is not free. This has not
been tried on a Mac.

## Details

Found by the 2026-10-22 prioritized persistent review, group `030-tunnels-1of2` (finding f1).

Suggested bucket: highest (an untrusted remote host chooses a port and receives credentials and traffic Mac programs
meant for their own local service; a write to the wrong database is a state-changing action on the wrong target). If
triage reads the warning as already accepting this, what remains is that the warning's "when free" promise is false.

Confidence: likely. Confirmed from source: the bind and its fallback, and that mio 1.2.4 sets `SO_REUSEADDR` before
every bind on non-Windows platforms. Unverified premise: macOS (XNU, BSD semantics) lets a same-uid socket with
`SO_REUSEADDR` bind `127.0.0.1:P` while another socket holds `0.0.0.0:P` or a dual-stack `[::]:P`, and then routes
connections to `127.0.0.1:P` to the more specific binding. Nothing was run on a Mac. On Linux the same bind fails with
`EADDRINUSE`, which is why Linux tests would not show it.

Mechanism:

- `bind_listener` (`crates/farhelm-helm/src/tunnels.rs:229-235`) calls
  `tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port))` and falls back to port 0 only when that bind fails.
- tokio's bind goes through mio 1.2.4 (`Cargo.lock:3723-3725`), whose `TcpListener::bind` sets `SO_REUSEADDR` on every
  non-Windows platform before binding (`mio-1.2.4/src/net/tcp/listener.rs:74-82` in the cargo registry; its comment says
  it skips this on Windows because it allows "socket hijacking"). Recording checked this source.
- With `SO_REUSEADDR`, BSD's bind conflict check is exact-address, so per the reviewer the specific loopback bind
  succeeds while a wildcard listener of the same user holds the port, and the fallback never runs.
- SPEC.md "Terminal experience" (the tunnel bullet, around line 1944) says the tunnel uses "the remote port number when
  free and otherwise an available port"; SPEC.md "Local authority and trust between hosts" (around line 3231) has the
  warning say the same.

Scenario: Docker Desktop publishes Postgres on `*:5432`, or `next dev` listens on `[::]:3000`. A remote session's agent
prints `http://localhost:3000/`, and the user, with tunnel consent on, Cmd-clicks it or answers Tunnel and open. The
tunnel binds `127.0.0.1:3000`, and until the session is deleted, Farhelm quits or the chip is closed, every Mac client
connecting to `127.0.0.1:3000` is relayed to the remote host's loopback.

How to verify, on a Mac: run `nc -l 0.0.0.0 3000` (or `python3 -m http.server 3000 --bind 0.0.0.0`), then bind
`tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 3000))` in a small Rust program. If it succeeds and
`curl 127.0.0.1:3000` reaches the new listener, the premise holds.

Fix sketch: bind the preferred port without `SO_REUSEADDR` (for example `tokio::net::TcpSocket` with
`set_reuseaddr(false)`, or `socket2`), so BSD's wildcard conflict check applies and the bind falls through to a
kernel-chosen port. Also consider refusing the preferred port when an IPv6 loopback or wildcard listener answers on it,
since Mac clients resolving `localhost` may try `::1` first. Keeping `SO_REUSEADDR` for the port-0 fallback is fine.

Possible cover, for triage to judge (the reviewer's `proposed_drop`): `TRIAGE_OUTCOMES.md`,
"tunnel-port-can-divert-local-traffic.md" (2026-10-18, `fix spec+code`, PR #2364) triaged same-port diversion in
general. The user called the port behavior "aids, not safeguards" and chose a warning sentence. That record explicitly
lists as unverified "whether address reuse on macOS lets a tunnel take 127.0.0.1:port while another program holds the
same port on all addresses", which this finding answers from source. The warning that decision added says a tunnel uses
the remote port "when free, so a Mac program expecting that port may reach the remote host instead"; it is conditioned
on the port being free, and hijacking a running local service is a different consequence from taking a free port. The
same SPEC.md section says its acceptances never relax the remote-host boundary. Related but separate:
`tunnel-can-take-standalone-helm-port.md` (a tunnel on the helm's own port) and
`removed-host-strands-tunnel-listeners.md`.
