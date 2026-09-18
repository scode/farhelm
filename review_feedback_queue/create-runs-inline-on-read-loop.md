# CreateSession runs its full validate-and-launch inline on the read loop

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Creating a session stalls everything else on the same connection — lists, stops, attaches across every open tab — for
the whole creation, because creation is the one slow operation that runs inline instead of in the background.

## Details

Source: pre-pr-review-swarm, area supervisor-handlers, 2026-09-17. Confidence: definite. Two lenses agreed
(correctness-systems p2, correctness-state-lifecycle p2). Coordinator confirmed the dispatch shape and the slow work
inside.

Every slow handler is spawned onto a tracked task through `spawn_admitted` — except create. `handle_create_session`
takes no task set and no permit (470-495) and is awaited inline at both dispatches (full-authority 2735-2756, restricted
via 3163), while the neighboring arms (list, browse, stop, restart, delete, archive) all receive `ctx.tasks`. The
inlined work is slow: `create_session` awaits validation with cwd checks, store operations, and `launch_session`
(core.rs:5720-5793: spec writes, tmux spawns, systemd probes). Both dispatches run on the connection's shared read loop,
and every browser tab rides one connection (2121-2123) — so one create, especially against slow storage or a wedged
tmux, stalls every pending request on that connection, and create escapes the admission bound every sibling honors.

Suggested fix: spawn the create onto a tracked admitted task like every sibling — give the handler the task set, run
validate-and-launch under admission, reply on completion — preserving the current reply shapes (internal-vs-refusal
mapping), just delivered asynchronously.
