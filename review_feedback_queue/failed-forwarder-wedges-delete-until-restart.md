# One failed output-forwarder join wedges delete and archive until restart

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

After a single internal cleanup failure, a session can never be deleted or archived again — every attempt is refused
with "still crossing its safe shutdown boundary" even though nothing is running — until the supervisor restarts.

## Details

Source: pre-pr-review-swarm, area supervisor-teardown, 2026-09-17. Confidence: possible (trigger is a rare forwarder
panic).

`record_forwarder_join` (terminals.rs:1489-1508) inserts a `Failed` output-reap entry that is NEVER cleared (no clearing
path anywhere: the only other registry touches treat `Failed` as terminal — return `Err` / keep), and
`has_output_reap_for_session` (terminals.rs:1515-1540) treats it identically to in-progress cleanup (`Failed` entries
are never settled out; the final `any` counts them). So one forwarder join failure (panic/cancel) makes teardown
(teardown.rs:351-355, 727-731) refuse delete AND archive forever with "still crossing its safe shutdown boundary" though
nothing is running — heals only on restart. Attach permanence is documented as intended; delete/archive permanence
follows mechanically and is undocumented — and delete destroys the tmux session the overlap barrier exists to protect,
so the guarded hazard arguably no longer applies.

To verify, force a forwarder join failure (panic/cancel) for a session, then delete or archive it: both refuse
permanently.

Suggested fix: distinguish `Failed` from in-progress `Reaping` in the whole-session teardown check (log loudly and
proceed with the tmux kill), or clear `Failed` once the session's tmux existence is gone.
