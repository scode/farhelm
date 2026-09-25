# A create retry can duplicate an agent on hosts without systemd

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In rare crash cases on a Mac or a Linux host without systemd, a retried create starts the agent again while the first
attempt's background processes keep running.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F5 / COR-RETRY-DUPLICATE-NO-MANAGER`, tagged **possible**. Anchors and title: `service/core.rs:7991-8059`,
`service/core.rs:7895`, `service/core.rs:8552`, `service/core.rs:9131-9158`, `service/core.rs:5968-6004` — Without a
systemd user manager, a create retry can launch a second agent beside the first attempt's surviving processes

A create can carry an _intent key_, which makes retries idempotent. The supervisor records a _reservation_ for the key,
and a retry of a pending reservation must decide whether the first attempt launched anything: if it did, replay that
session; if not, launch again. `reserved_launch_evidence` (`core.rs:7991-8059`) makes the call from these sources:

- a pane or conversation report recorded on the row;
- a generation-0 systemd scope, checked only when a user manager is available;
- a generation-0 launch sentinel, which the shim writes only when exec fails, so a successful launch never leaves one;
- tmux reporting that the session exists right now.

On macOS, and on Linux without `systemd --user`, the scope source is skipped. Reload's settlement of pending
reservations (`core.rs:5968-6004`) uses the same sources and has the same gap.

Now suppose the supervisor crashed after `tmux new-session` but before `ConfirmRunning` recorded the pane, and the tmux
server or session was then also lost. The first agent may have exec'd fine and started background daemons, such as a dev
server or MCP server. Those daemons still carry the `FARHELM_SESSION_ID` environment marker. Every source above reads
absent, so the answer is `Absent` (`core.rs:7895`). The retry takes over the reservation and launches again under the
same session identity (`core.rs:9131-9158`). Unlike restart, `launch_reserved` does not run the marker-based leftover
sweep before spawning, so the new agent runs alongside the first attempt's survivors. SPEC says one intended create
yields one session, and that leftover descendants are reaped before a relaunch, never alongside it. If the retry is
instead refused by validation, the row is deleted and the survivors are left with no session that owns them.

This is marked possible and is rare: it needs a crash inside the launch, tmux loss, and a surviving daemonized
descendant, all on a host without a scope manager. Candidate fixes:

- Add a `/proc` marker scan as evidence, in both the retry check and reload's settlement. Any match counts as present; a
  scan that fails counts as unresolved.
- Reap marker-carrying processes before a retry launches.
- Durably record "spec published" before the tmux call. The shim deletes the spec once it runs, so "published but now
  missing" proves it ran and can be treated as present.
