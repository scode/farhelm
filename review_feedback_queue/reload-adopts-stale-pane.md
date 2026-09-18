# Reload adopts a stale dead pane as the new generation's terminal

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

After the supervisor restarts, a session whose last restart crashed can wrongly show as normally exited — with someone
else's exit code — instead of "unknown, retry me", and that wrong answer gets permanently recorded.

## Details

Source: pre-pr-review-swarm, area supervisor-lifecycle, 2026-09-17. Confidence: definite (correctness-edge-inputs p1).
Coordinator confirmed the insert, the classifier arms, and the commit path.

A row left by a restart that crashed between opening its generation and confirming it (generation > 0, no pane,
`Launching`) hits the stale-pane rule at reload (`crates/farhelm-supervisor/src/service/core.rs:5044-5066`): a dead pane
found by tmux session name is recognized as the previous run's pane and no exit transition is proposed — the row
correctly stays `Launching`, and the comment promises it lists as `Unknown`. But the pane is then unconditionally
inserted into `found_panes` (core.rs:5084), which feeds the rebuilt entry's terminal (core.rs:5141). From there:
`session_status` matches `(Launching, Some(dead pane))` on the generic arm (status.rs:193-217) and reports `Exited` with
the old run's code (the `(Launching, None) => Unknown` arm, status.rs:231, needs no pane); then `observation()` yields
`ObservedExit` (status.rs:497-517, only Launching-with-no-pane excluded), which `Transition::apply` commits from
`Launching` (store.rs:526-528). The crash window is real process death during the relaunch task. This is the exact
misattribution the comment forbids ("recording its exit against this generation would attribute a death to a launch that
never happened").

Suggested fix: skip the `found_panes` insert in the stale-pane branch (continue like the archived branch), leaving the
entry terminal-less — the restart-gap shape that lists `Unknown`, stays `Launching`, and lets a retry rebuild a fresh
terminal. Constraint to preserve: a prior fix deliberately keeps panes flowing into `found_panes` on the sentinel
branches so `Attach` keeps working and the tmux session is not leaked (core.rs:4937-4945) — keep enough addressing for
attach/teardown without letting the classifier treat the pane as this generation's evidence. See also the live-operation
sibling `ambiguous-restart-misattributes-exit.md`.
