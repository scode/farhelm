# Supervisor positional-pin and named-restart tests omit AgentKind::Custom (no test witnesses CustomPinCorrected, quiet_event=None, or custom_reporting=false clear_capture)

Reviewed commit: 7164b111339f33f9ec0b296488c38102191f6d9b

## TLDR

Custom agents share the supervisor's positional process-pin admission, ancestor-correction notification, and
named-restart capture-clearing code, but none of those supervisor behaviors are tested for custom agents. Regressions in
custom-agent pin correction, notification recording, rival-report refusal, or clearing old capture when reporting is
disabled would pass the supervisor test suite unnoticed.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core/tests/claude_pin.rs:26-49`.

Commit `7164b111339f33f9ec0b296488c38102191f6d9b` wired `AgentKind::Custom` into
`Supervisor::report_positional_conversation` (`crates/farhelm-supervisor/src/service/core.rs:15731-15740`,
`crates/farhelm-supervisor/src/service/core/vendor/positional.rs:65-73,101-103,139`), added
`NotificationKind::CustomPinCorrected` (`crates/farhelm-supervisor/src/service/notifications.rs:98,191,238`), and
updated `clear_capture` (`core.rs:12718-12729`) to clear prior capture on a named `Restart with` when
`executable_launch.custom_reporting() == Some(false)`.

However, `crates/farhelm-supervisor/src/service/core/tests/claude_pin.rs` (`L26-49,834-923`) defines `PinCase` fixtures
only for `CLAUDE`, `MUSE`, and `ANTIGRAVITY`, and omits `AgentKind::Custom` entirely. Across the entire repository at
`1b3fe49af86979aebc2719965682a17f8c6366ac`:

1. Zero tests reference `custom_pin_corrected` or `CUSTOM_PIN_CORRECTED_TEXT` (`git grep` returns only
   `positional.rs:72` and `notifications.rs:191,238`), so custom-agent ancestor correction and once-per-launch
   notification persistence are completely unwitnessed (the e2e test in
   `crates/farhelm/tests/e2e/hook_identity.rs:2215-2325` also does not distinguish ancestor correction from dead-pin
   release or check notifications, as noted by `g47/l1`).
2. No supervisor unit test exercises `vendor::positional::CUSTOM` for descendant/rival/reused-PID refusal, dead-pin
   release only outside the incoming chain, unknown-liveness and write-failure rollback,
   `row.launch.custom_reporting() == Some(false)` refusal under `report_positional_conversation`, or
   `CUSTOM.quiet_event == None` (verifying that a custom report carrying `hook_event_name: Some("Stop")` is NOT silently
   swallowed on rival refusal).
3. No test exercises `core.rs:12718-12729` (`clear_capture` triggered by
   `executable_launch.custom_reporting() == Some(false)` on a named `Restart with`, including restoration on
   `abort_relaunch`).

How to verify: Inspect `crates/farhelm-supervisor/src/service/core/tests/claude_pin.rs:26-49,834-923` and grep for
`custom_pin_corrected`, `CUSTOM_PIN_CORRECTED_TEXT`, and `custom_reporting` across
`crates/farhelm-supervisor/src/service/` and `crates/farhelm/tests/`.

Fix: Add a `CUSTOM` `PinCase` (using a `SessionLaunch::Agent` or `Command` fixture with `PositionalReporter::Custom`
reporter argv) to `claude_pin.rs` covering ancestor correction + `custom_pin_corrected` notification,
rival/descendant/dead-pin/write-failure behavior, `quiet_event: None` refusal of `hook_event_name: "Stop"`, and
`custom_reporting: Some(false)` refusal; add a supervisor test covering `Restart with` + `named_start` clearing capture
when `custom_reporting` is `Some(false)`.

Reviewer metadata (g42b_custom_agents_sup_service/f3): suggested bucket: other — test-coverage gap in maintainer tests;
does not itself directly cause security compromise, data loss, or degraded user experience.; confidence: confirmed
(verified directly from source and repo-wide grep at `1b3fe49af86979aebc2719965682a17f8c6366ac`).; possible cover: none
(complements `g47_custom_agents_e2e_a/l1`, which covers the e2e test in `hook_identity.rs`, and
`g55_positional_hooks_shared/f2`, which covers `CUSTOM_PIN_CORRECTED_TEXT` wording).

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g42b_custom_agents_sup_service/f3`.
