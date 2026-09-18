# Untracked mutations leak permit, claim, and fence against wedged tmux

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

If tmux wedges, a handful of stuck stop/delete/archive operations can freeze the whole supervisor — every slow request
afterwards waits forever, with no recovery short of restarting.

## Details

Source: pre-pr-review-swarm, area supervisor-handlers, 2026-09-17. Confidence: definite (correctness-state-lifecycle
p2). Coordinator confirmed the untracked tasks, the untimed tmux waits, the permit count, and the shutdown blind spot;
the restater corrected stop's tracked/untracked split and the coordinator verified the correction.

Stop (914), delete (1242), and archive (1354) each run their mutation in an untracked `tokio::spawn` holding an
admission permit, a lifecycle claim, and (for delete) a fence. (For delete and archive only the reply waiter is tracked;
for stop the untracked mutation delivers the reply itself while the tracked task merely awaits it to log a join failure,
1213-1217.) Connection shutdown aborts tracked tasks only, after a bounded wait (connection.rs:672-683). The mutations
await tmux down the untimed path — stop's `pane_process` (948) funnels into `run` → `run_bytes` and its bare `.output()`
with no timeout (tmux.rs:2216-2222), and the teardown probes take the same road — while sibling tmux operations wrap
identical awaits in explicit timeouts (tmux.rs:2129, 2186). There are 8 permits process-wide, sized explicitly to bound
tmux subprocesses (core.rs:1157-1179), and a survives-disconnect wedged tmux is contemplated as survivable
(connection.rs:664-666). Eight parked mutations therefore exhaust the only admission bound: every subsequent slow
request parks forever, pinned sessions' lifecycles and fences never release, and the disconnect that should reap them
cannot see the tasks.

Suggested fix (any one closes the leak): track the mutation tasks so shutdown aborts them; wrap the mutation's tmux
awaits in the sibling operations' timeouts so a wedge fails the mutation instead of parking it; or bound the permit hold
with a request-failing timeout. The tmux timeouts match the existing pattern most closely.
