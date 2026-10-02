# Automatic tab cleanup stops status sampling for every session during process shutdown

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

Automatic cleanup of an exited terminal tab can leave every session on that host showing stale status for seconds while
background processes stop.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F4 / COR-TAB-STATUS`, reviewer `correctness_state_lifecycle`, pass 1. Confidence: **definite**. Review disposition:
**would fix**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-supervisor/src/service/ticker.rs:1834`. Recorded from the completed
review without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording:
Related to `tab-reap-budget-starved-by-failures.md`, but this finding concerns normal slow cleanup blocking the sampler,
not failed closes exhausting the reap budget.

When an additional terminal tab's shell exits, the supervisor's periodic status task waits for that tab's entire cleanup
before sampling live agent screens. The close operation tries to acquire the session's lifecycle lock without waiting,
but once it succeeds, the caller awaits both process-cleanup passes and removal of the terminal window. A background
process that survives the shell and ignores TERM and HUP can consume the normal five-second shutdown grace period. The
sampler can close four tabs serially, and the pane-death notification path waits through the same cleanup.

Other sessions on that host can consequently retain stale running, idle, waiting, and recent-work state for
seconds—roughly twenty seconds for four resistant tabs. Asking for the session list does not repair this because
listings use the previously sampled status. Keep automatic cleanup owned and bounded, but let it run independently of
status sampling, retaining each session's lifecycle guard and preventing duplicate close jobs. A regression should hold
one tab's cleanup in its shutdown phase and prove that another session still receives a fresh sample. This addresses the
status-isolation requirement without weakening process cleanup.

Additional original references: `crates/farhelm-supervisor/src/service/core.rs:12450-12556` and `12810-12930` (cleanup),
`crates/farhelm-supervisor/src/service/ticker.rs:1317` (sampling), and `sweep.rs:50,1090-1110` (shutdown grace).
