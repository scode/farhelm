# Attaching a tab can pause input across its host

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

Opening or reconnecting a terminal tab while its session stops, restarts or deletes can delay typing and unrelated
requests on that host for up to two seconds per attachment.

## Details

Suggested bucket: `other`. Confidence: confirmed static call chain and production entry path; not runtime reproduced.
Provenance: retained `supervisor-transport-dispatch-1/f1`, second-turn synthesis. Proposed drop: none. The initial
review suggested `high`, then revised it because frequency and aggregate delay are unmeasured and the established
consequence is bounded. Existing output forwarders can continue, and other hosts remain independent.

The connection's sole reader awaits `handle_control` (`crates/farhelm-supervisor/src/service/connection.rs:691`), which
awaits tab attachment (`service/handlers.rs:2713–2727`). Attachment waits inline for the session lifecycle claim
(:1278–1296), with a two-second timeout (`service/core.rs:335`). While it waits, the reader cannot dispatch terminal
data, Resize, Detach, ListSessions or AgentResponse frames.

Ordinary Stop holds that claim from `handlers.rs:770` through process teardown; Restart claims it at `core.rs:10461` and
stops the agent at :10747; Delete takes it at `handlers.rs:1123` and reaches `reap_process_tree` through
`teardown.rs:430`. `sweep.rs:50,1134,1676` permits five seconds of normal SIGTERM grace. No hung dependency is needed,
though grace ends earlier when processes exit promptly. UI tab paths and automatic reconnect paths
(`crates/farhelm-ui/src/session_view.rs:1887–1903`; `assets/terminal.js:1684–1758`) reach the shared supervisor client
through `crates/farhelm-helm/src/terminal.rs:190–202`.

Refuse a contested tab attachment immediately or move its wait off the reader while preserving route ownership. A
focused check should hold a session's lifecycle claim, send its tab Attach, then send input or a list request on the
same connection and verify that the latter progresses before the claim is released.

Possible cover: none exact. SPEC.md, “Waiting between operations on one host,” explicitly excludes process grace periods
from work terminal operations may wait on. “Slow hosts” preserves that exclusion for work long on a healthy host.
TODO.md's Planned “Keep session creation off the connection read loop” and the ledger's creation/reconciliation items
cover those requests, not tab attachment. The completed “stop-admission-blocks-terminal-reader.md” fix concerns
management-capacity admission, not this lifecycle lock. BUGS.md's idle-input backlog has a different trigger. The
rare-glitch filter does not cover blocking unrelated operations; the sub-second human-race filter does not fit automatic
reconnect or a multi-second shutdown window; the hung-dependency filter requires a trigger absent here.
