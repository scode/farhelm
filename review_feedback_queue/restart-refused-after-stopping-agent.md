# A consented restart can stop the agent then refuse to relaunch

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Occasionally a confirmed restart only stops the agent and reports that nothing happened.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0508-2b597e9-5788` (audit of Area 6, supervisor session state machine) as
`F2 / COR-RESTART-OFFER-AFTER-STOP`, tagged **possible**. Anchors and title: `service/core.rs:9861-9880`,
`service/core.rs:9980`, `service/core.rs:10149`, `store.rs:3969-3976`, `service/capture.rs:928` — A consented restart
can stop the agent and then refuse to relaunch because the offer changed during the stop

A restart comes in one of three modes: Resume the captured conversation, run a fallback template, or start Fresh. The
mode must match the session's current _restart offer_. The offer is derived from the session's _capture state_: the
conversation id the supervisor has recorded for the agent (_captured_), whether that identity was judged ambiguous, and
an ownership version counter. Together these three values are the _offer basis_. `restart_session` runs one capture pass
(`capture_now`), reads the durable snapshot, and checks the requested mode against it (`core.rs:9861-9880`). If the
agent is still running and the caller consented (`stop_if_running`), it then stops the agent: SIGTERM, a grace period,
and a process-tree sweep (`stop_live_agent`, `core.rs:9980`). Only after that does the relaunch call
`store.begin_relaunch`, which re-reads the three columns atomically and returns `OfferChanged` if any of them moved
(`store.rs:3969-3976`). The restart then fails with a Conflict saying "nothing was relaunched — refresh the session and
re-present the offer" (`core.rs:10149`).

Capture state can change in the gap between those two checks. The periodic capture pass (`capture_pass`,
`service/capture.rs:928`) and the conversation-report handler take neither the session's _lifecycle claim_ (the
per-session lock that Stop, Restart and Delete hold) nor the capture lock across the restart. So during the multi-second
stop, a capture commit or report can land and change the basis. Examples: a Claude session whose capture window closes
gets its id claimed, a Codex/Grok locator becomes ready as the transcript flushes on shutdown, or a Pi/OMP agent reports
on exit. The user then sees a Conflict error that says nothing happened, but the agent has already been killed and the
session is left stopped. An agent that restarts itself with `--stop-if-running` stops and never comes back. Nothing is
permanently lost, since a second restart against the new offer works.

This is marked possible because it depends on a capture commit or report landing inside the stop window. That was
established by reading the code, not reproduced. Suggested fixes, strongest first:

- Hold the capture coordination lock (or a per-session capture fence) from the snapshot read through `begin_relaunch`.
- When this request already stopped the agent and the offer only improved (FreshOnly to Resume), relaunch against the
  new offer.
- At minimum, say in the error that the agent was stopped.

Restater note: The "contradicts the docs" point is only partly accurate. The `restart_session` docs
(`core.rs:9710-9717`) say the listed refusals, including the mode-against-offer check, run before any side effect, and
`OfferChanged` is a late repeat of that check. But the same paragraph admits that a restart can fail after stopping the
agent, and says the stop annotation is then preserved by `abort_relaunch`. `OfferChanged` returns before a generation is
opened, so no abort runs, and the session simply remains in its recorded stopped state.
