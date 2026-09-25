# The kill sweep can claim the supervisor or its tmux server

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

In the affected setups, deleting one session or closing a tab could freeze the supervisor or take down every session on
the host.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F16 / COR-SELF-SWEEP`, tagged **possible**. Anchors and title: `service/sweep.rs:419-451`, `service/sweep.rs:251-260`,
`service/sweep.rs:595-599`, `tmux.rs:1954-1958` — Nothing stops the sweep from claiming the supervisor itself or its
private tmux server

The kill sweep's starting set is every same-user process whose environment carries the targeted markers
(sweep.rs:419-451, sweep.rs:251-260), plus the pane root (sweep.rs:595-599), expanded down the parent-pid tree. There is
no exclusion for the supervisor's own pid (`std::process::id()`), its ancestors, or the pid of its private tmux server.
Nothing at supervisor startup or in `TmuxDriver::command` (tmux.rs:1954-1958) removes `FARHELM_SESSION_ID`,
`FARHELM_AGENT_ID` or `FARHELM_TAB_ID` from the supervisor's own environment, and tmux copies the environment it starts
with into its global environment.

A supervisor can end up carrying one of its own sessions' markers without being that session's descendant in two ways:

- It is started by hand from inside one of its own tabs or panes, against the same state directory. The desktop app can
  do this too (crates/farhelm-ui/src/desktop.rs:1306-1320), since it spawns a supervisor when none is answering and the
  child inherits the app's environment.
- A tab's rc files run `systemctl --user import-environment` with no arguments, or
  `dbus-update-activation-environment --systemd --all`. That copies the tab's markers into the systemd user manager's
  environment, which `farhelm-supervisor.service` inherits on its next restart; the unit has no `UnsetEnvironment=`.

Deleting that session (whole-session target) then claims the supervisor itself and, if the supervisor started it, the
private tmux server. The tmux server's parent-pid closure is every pane of every session on the host. A SIGSTOP sent to
itself freezes the supervisor permanently, because the SIGCONT guard would have to run inside the stopped process. The
supervisor installs no SIGTERM handler, so the SIGTERM phase may also kill it mid-delete.

One Delete, tab close or Stop could therefore kill every agent and terminal on the host plus the supervisor. That is the
opposite of SPEC's "must not accidentally affect the wrong object".

The premise is a supervisor that is not a descendant of the session yet carries its markers; this is environment
contamination and uncommon. In `snapshot_proc`/`enumerate_tree`, never admit or signal the supervisor's own pid, its
ancestors, or the private tmux server's pid, and never expand the parent-pid walk through the tmux server; report a
claim of any of them as an error. One reviewer also suggested scrubbing the markers at startup. Another cautioned that a
supervisor legitimately nested inside an outer session (common when an agent develops Farhelm under Farhelm) needs those
markers so the outer session can reap it, so the exclusion is preferred.

For the user, in the affected setups, deleting one session or closing a tab could freeze the supervisor or take down
every session on the host.
