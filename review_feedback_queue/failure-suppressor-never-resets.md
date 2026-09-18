# Failure-log suppressor never resets and mixes unrelated failure kinds

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

When a host connection keeps failing the same way, the log goes quiet after three lines and stays quiet — even across
recoveries. A new outage identical to an old one can log nothing at all, and the "suppressed N lines" counts can mix
failures from unrelated subsystems.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible, low. Coordinator verified.

`RepeatedFailure` (manager.rs:717-723) persists for the actor's whole life: initialized once (manager.rs:1577), touched
only inside `note_failure` (manager.rs:4010), never reset on success — and one instance serves every failure kind (dial
at :3044, skew at :3154, refresh at :3558, contested at :3674, cache write at :3703). Three consequences: (a) an outage
with byte-identical text to a previous, already-suppressed outage logs nothing from its first line — the "log 3 then
suppress" budget (REPEATED_FAILURE_LOG_LIMIT, :715) was spent by the earlier outage; (b) the `suppressed` count a new
failure reports can include suppressions from before a recovery in between; (c) kinds cross-talk — a contested-collision
summary displaces a dial-failure run and inherits its count, and vice versa.

Bounded: phase transitions still log via `publish_refresh`'s phase check, so a recurring outage is never invisible, only
its per-attempt detail — but the trail degrades exactly when it is most needed (a flapping host repeating one error).
Possible, low.

Suggested fix: reset the suppressor on success (clear in `serve` on first `RefreshHealth::Ok`, and on leaving
`Unreachable`), and/or key suppression per failure kind (separate small states for dial/skew/refresh/contested) so
counts are never attributed across kinds.
