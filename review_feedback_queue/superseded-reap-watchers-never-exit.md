# Superseded output-reap watchers never exit, leaking a task per churn cycle

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

When terminal handover churns repeatedly while tmux is unresponsive, the supervisor slowly accumulates dead background
tasks that never exit, growing memory use for the rest of its lifetime.

## Details

Source: pre-pr-review-swarm, area supervisor-teardown, 2026-09-17. Confidence: possible, low (needs repeated churn on
one terminal while tmux is wedged).

Each `track_output_reap` (terminals.rs:1450) overwrites the key's registry entry and spawns a watcher task that exits
ONLY when ITS receiver settles (:1460-1469). Reapers retry forever by policy, so once a newer barrier overwrites an
older one for the same `(session, terminal)` key — repeated takeover/replace churn on one terminal while tmux is wedged
— the older watcher's terminal transition is a guaranteed no-op (the `same_channel` check at :1471-1474 fails), yet the
watcher still waits forever on a completion nobody will ever read. Unbounded dead-task accumulation (task + receiver +
key per churn cycle) under a permanently failing substrate, within process lifetime (task abort drops the sender and
exits the watcher, so shutdown bounds it). The reapers themselves must stay (they own live clients); only the superseded
watchers are pure overhead with a safe exit available.

Suggested fix: let a superseded watcher exit — when its wait extends, re-check whether the registry still holds its
channel and return early if superseded, since its transition is already known to be a no-op.
