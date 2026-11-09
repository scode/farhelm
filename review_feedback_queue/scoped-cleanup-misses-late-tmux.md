# A tmux server started after inspection can escape scoped cleanup

Reviewed commit: 5f235d92f7184cdf949cf9d45e86dc19e71ff4df

## TLDR

Stop, Restart, Delete or closing a terminal tab can report completed cleanup while a job the session started inside tmux
keeps running. The job can survive if its tmux server starts after cleanup's inspection but before termination.

## Details

Suggested bucket: **high**. Confidence: **source-confirmed conditional interleaving**, without runtime reproduction or a
frequency claim. No security or user-work-loss consequence is established; the demonstrated consequence is surviving
jobs and possible overlap with a new run.

`crates/farhelm-supervisor/src/service/sweep.rs:1522-1549` inspects every selected scope once, then asynchronously ends
the scopes and can omit the portable process sweep. `scope.rs:1150-1166` inspects only the process numbers returned by
one membership snapshot; `scope.rs:146-188` examines their current roles/images. Neither observation quiesces the live
agent or shell. Scope cleanup has further awaits and a normal termination grace (`sweep.rs:1797-1846`). A normal
systemd-linked tmux server can start during this interval and move its hangup-immune pane program into a sibling
`tmux-spawn-*.scope`. Killing the original scope and observing its explicit roots gone then proves nothing about that
migrated program.

The added regression at `crates/farhelm/tests/e2e/session_lifecycle.rs:5860-6030` establishes the stock-tmux migration,
readable inherited marker and hangup-immune job premises, but fully starts the server before Stop. It does not protect
the observation-to-termination interval. Upstream
[tmux 3.4 pane creation](https://github.com/tmux/tmux/blob/3.4/spawn.c#L368-L396) and
[scope creation](https://github.com/tmux/tmux/blob/3.4/compat/systemd.c#L102-L171) corroborate the migration contract.

At this revision, `SPEC.md`, "Lifecycle operations", lines 1840-1859 requires ordinary tmux pane programs to end with
their owner. `SPEC_impl.md:5052-5061` explicitly distinguishes that behavior from a deliberate scope escape. This route
needs no supervisor crash, recycled process number or pair of independent faults. The rare-glitch filter does not cover
reporting successful cleanup while processes survive. The first-starter ownership decision concerns which work must end,
not permission to omit it.

Keep absence evidence valid until the target is quiescent, or retain the sweep when that cannot be established. Merely
adding another live snapshot leaves the same kind of interval.

Possible coverage: no exact existing queue, ledger, accepted specification, known bug or unlanded plan covers this
route; Planned was empty. The scoped-cleanup plan already landed and its pre-kill check does not close this interval.

Provenance: trailing review run `ee465fa5-0b29-4ce7-b1fe-8fabdb679cfe`, records `cleanup-scope/f1`, `cleanup-sweep/l2`
and `cleanup-callers/f7`. Original events, corrections and delivery evidence remain in that run.
