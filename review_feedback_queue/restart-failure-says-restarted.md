# A failed restart tells the client "session restarted" although nothing restarted

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

When a restart fails while detaching the old view, the app is told the session restarted even though the old session is
still the live one — so it can show a fresh start that never happened while the previous run keeps going underneath.

## Details

Source: pre-pr-review-swarm, area supervisor-lifecycle, 2026-09-17. Confidence: definite (correctness-systems p3).
Coordinator confirmed the ordering and the abort path. (An earlier second-pass claim that the client gets no notice at
all was rejected at confirm: the notice is sent unconditionally — this finding is the corrected, opposite defect in the
same lines.)

`detach_for_restart` computes both failure signals — the forwarder-join cleanup result (core.rs:8793) and the
reaping-barrier check (core.rs:8796) — then unconditionally sends `"session restarted"` (core.rs:8798) before checking
either (core.rs:8799-8804). A failure returns `RelaunchFailure::definitive` (core.rs:8018-8021), so the caller runs
`abort_relaunch`, restores the previous run's outcome, and republishes the old terminal: nothing restarted. A client
acting on the notice — reattaching and assuming a fresh agent, resetting transcript state, or automation skipping a
needed retry — operates on a false premise. The codebase already has the correct shape: the archive path sends
`"detached during a failed archive: ..."` when its teardown fails.

Suggested fix: move the two failure checks above the notify — success sends `"session restarted"` as today;
cleanup/reaping failure sends a failure-shaped notice stating the attach ended but the restart did not happen, naming
the cause and prompting reattach. Do not skip the notice on failure: the attachment is already removed from the map, so
the client must still be told to reattach.
