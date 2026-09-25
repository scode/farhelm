# Superseded launch specs are never removed while the session exists

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After restarting a session whose earlier launch never started, the old command line and credentials stay on disk until
the session is deleted.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F20 / COR-SUPERSEDED-SPEC`, tagged **definite**. Anchors and title: `service/launch_artifacts.rs:399-412`,
`service/core.rs:10336-10627`, `store.rs:3925` — Launch specs of superseded or unconsumed launches are never removed
while the session exists; the startup sweep relies on a restart cleanup that doesn't exist

Each launch of a session writes its own launch spec at `<state_dir>/launch/<id>.<generation>.json`. "Generation" is a
counter that goes up on every restart, so each restart writes a new file name. The spec is mode 0600 and holds the
agent's full command line, where users put API keys, plus the session's spawn token, a bearer credential. SPEC_impl
"Runtime state" promises three cleanups: the shim deletes the spec once it has read it, creation removes it if the
session never starts, and the supervisor sweeps leftovers at startup.

The startup sweep, `sweep_launch_dir` (`service/launch_artifacts.rs:399-412`), removes a `.json` spec only when its
session no longer exists. For a live session it keeps every generation's spec. Its comment explains why: "the restart
that superseded it removes its own predecessor". No restart code does that. `relaunch`, `relaunch_into_terminal`
(`service/core.rs:10336-10627`) and `publish_relaunched` only write and, on their own failure paths, remove the _new_
generation's files. `begin_relaunch` (`store.rs:3925`) only bumps the generation in the database. `spec_path_for_launch`
is only ever called for the current generation. The only code that enumerates older generations is Delete
(`remove_launch_artifacts_for_session`).

A spec survives unread whenever the login shell never reached the shim. That happens when an rc file `exec`s another
shell or exits, when the user stops or restarts while a slow rc file is still running, or when tmux or the host dies
first. Those are exactly the situations in which a user presses Restart. The new generation's spec is then consumed
normally, while the old generation's spec stays on disk for the life of the session. This breaks the SPEC_impl cleanup
promise. The exposure is limited to the same Unix account, which SPEC treats as the security boundary. The practical
cost is credential hygiene, not privilege escalation.

The suggested fix: once a relaunch is confirmed and the prior run has been reaped, best-effort remove the specs and
sentinels of all lower generations. Alternatively, have the startup sweep compare each spec's generation with the row's
current generation. In either case, correct the sweep's comment. This finding covers the _earlier-generation_ files that
a restart leaves behind. F21 and F22 cover the _current_ generation's spec when an observer records a final outcome, and
F23 covers hidden staging copies.

Restater note: SPEC_impl's own sentence ("`launch/` holding one 0600 JSON spec per session") predates per-generation
naming and is itself stale. That is a documentation-only side point.
