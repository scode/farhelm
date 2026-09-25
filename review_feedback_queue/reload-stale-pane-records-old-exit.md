# A crashed restart is recorded with the previous run's exit

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

The session shows e.g. "exited (code 143)" for a relaunch that never started, instead of "unknown".

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F3 / COR-STALE-PANE-OLD-EXIT`, tagged **definite**. Anchors and title: `service/core.rs:5653-5693`,
`service/status.rs:501-516`, `service/ticker.rs:1034-1047` — After a crash mid-restart, reload attaches the previous
run's dead pane and the ticker records its exit against the new generation

Every launch of a session has a _generation_ number, which increases by one on each restart. Durable writes about a
launch are fenced by generation, so an observation about generation N cannot overwrite the record of N+1. Agent panes
use tmux `remain-on-exit`, so a pane whose process died stays around as a "dead pane" that still holds the exit code. A
restart stops the old agent, which leaves its pane dead. It then commits generation N+1 as `Launching` with an empty
pane column (`begin_relaunch`), and only afterwards writes the new launch file and respawns the pane. If the supervisor
process dies in between (an upgrade restart, a crash, SIGTERM; the relaunch task does not survive the process), the next
startup's reconciliation, `reload_sessions`, finds the old dead pane by its tmux session name.

Reload has a rule for exactly this shape (`core.rs:5653`): a dead pane found by name for a row with an empty pane and
generation above 0 belongs to the previous run, and recording its exit "would attribute a death to a launch that never
happened". It therefore commits no transition, and the row stays `Launching`, which lists as unknown. But the line after
that block, `found_panes.insert(row.id, (pane, state))` (`core.rs:5693`), still installs the old dead pane as the
published entry's terminal. On the first tick, the ticker sees a dead terminal (`ticker.rs:1034-1047`) and calls
`observe_entry`. With no sentinel and (in this variant) no wrapper-failure classification,
`observation(Launching, dead pane)` returns `ObservedExit` with the old pane's exit code (`status.rs:516`), tagged with
generation N+1. The generation fence accepts it, because the row is at N+1. `ListSessions` takes the same path.
`begin_relaunch` has already cleared the "stopped by user" annotation, so the durable record now says the new run ran
and exited with, for example, code 143 from the old agent's SIGTERM. Nothing corrects it later. Reviewers note the same
shape can also arise after an ambiguous tmux failure during restart, or when restoring the old outcome after a failed
restart itself fails.

The fix is to make the ticker and listing respect the rule reload already applies. Either reload publishes such a row
without a terminal, or `observation` ignores a dead pane for a `Launching` row whose stored pane is empty and whose
generation is above 0. With that change the session shows an honest unknown instead of a fabricated exit.

Restater note: On a host with systemd scopes, in the sub-case where the supervisor died after writing the N+1 launch
file, reload's earlier sentinel branch misclassifies the row as `Error` first (that is F4), so F3's ticker path applies
to the remaining cases: hosts without scopes, or a crash before the file was written.
