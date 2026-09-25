# A dead reused pane is treated as a definitive relaunch failure

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Under load, a restart whose new agent failed at once reports "restart failed" and reverts to the old status, hiding that
the relaunch ran.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F14 / COR-REUSED-PANE-DEAD-DEFINITIVE`, tagged **possible**. Anchors and title: `service/core.rs:10684` — A failed
relaunch into a reused pane treats "pane is dead" as "respawn never happened"

When a session's tmux pane still exists at restart, the relaunch reuses it: tmux's `respawn-pane -k` starts the new
agent in the same pane, and the pane keeps its id. If that tmux command reports failure, `unwind_failed_relaunch` has to
decide whether the relaunch took effect anyway. The failure could be a timed-out exchange that tmux had actually
applied. The answer matters. A "definitive" failure means nothing changed outside the supervisor, so the caller calls
`abort_relaunch` and puts the session back to its previous outcome, for example "exited, stopped by user". An
"ambiguous" failure keeps the new generation's `Launching` record for startup reconciliation to settle.

For a reused pane, the probe maps `PaneProbe::Owned(pane) => Some(!pane.dead)` (core.rs:10684). A pane that is alive
counts as "applied", which is ambiguous and handled correctly. A dead pane counts as "not applied", so the function
removes the launch spec, sweeps the process tree and returns `Definitive`. But the pane was already dead before the
respawn, because restart stops the old agent first. A respawn that applied and whose new agent failed immediately also
leaves a dead pane with the same id, for example a command-not-found exec failure or a `--resume` that exits at once.
The probe cannot tell these two cases apart. In the second case a relaunch really ran, but the session is reverted to
its old status. Any failure sentinel the new generation's launch shim wrote is left orphaned. The sentinel is the small
status file the shim writes when exec fails.

This needs a specific coincidence: `respawn-pane` reports failure after tmux applied it, which is most plausible for a
timed-out exchange under load, and the new agent exits before the probe runs. When it happens, the user sees "restart
failed" and the session's old status comes back, which hides that the relaunch ran and failed. The suggested fix is to
distinguish the two dead states before declaring the failure definitive. Compare the pane's process id or start time
with a probe taken before the respawn, or look for a sentinel or a consumed spec at the new generation, and treat
anything uncertain as ambiguous.
