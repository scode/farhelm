# Teardown's tmux calls run unbounded under the global lock

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

If tmux stops responding while a session is being deleted or archived, the whole supervisor freezes — every other
session's attach, input, and detach stall indefinitely behind the stuck operation.

## Details

Source: pre-pr-review-swarm, area supervisor-teardown, 2026-09-17. Confidence: possible (needs a wedged tmux server).

Three tmux calls — archive `kill_session` (teardown.rs:361), delete `has_session` (:767), delete `kill_session` (:799) —
go through `TmuxDriver::run` → `run_bytes`, which awaits `Command::output()` with NO timeout (tmux.rs:2212-2221), and
all three run while holding the map-wide `attachments` mutex (locked at :313 for archive, :669 for delete). A
wedged-but-alive tmux server (CLI blocks forever on the reply) stalls teardown's lock-held phase indefinitely. The stall
is GLOBAL, not request-scoped — every other session's attach/input/detach parks on `attachments`, and the caller's
lifecycle claim stays held, blocking stop/restart/tab-close and the ticker's reap on that session. The module docs
assume this phase is "one tmux round trip" and fast, and the codebase's own threat model already treats wedged tmux as
real (`capture_pane`'s deadline exists because "a wedged tmux must not park the sampler indefinitely") — the sampler is
defended, the teardown-under-global-lock is not.

Suggested fix: wrap the three calls in `tokio::time::timeout` and fail closed on expiry (row retained, retryable — the
existing `FailClosed` design already fits). Note `run_bytes` also lacks `kill_on_drop`, so a timed-out CLI would linger
until tmux answers; the fuller fix (timeout + kill in the driver) lives in tmux.rs, outside the finding area's scope.
