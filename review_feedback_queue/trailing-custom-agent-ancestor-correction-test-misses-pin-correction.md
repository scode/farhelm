# Custom-agent ancestor-correction e2e test does not distinguish ancestor correction from dead-pin release

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

The end-to-end test that claims to verify a custom agent parent can correct a child-first conversation report would
still pass even if custom-agent ancestor correction and its user-facing correction notification were completely broken.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/hook_identity.rs:2215-2325`.

In `crates/farhelm/tests/e2e/hook_identity.rs:2215-2325`
(`custom_agent_positional_report_corrects_to_the_parent_and_restarts_it`, introduced in
`7164b111339f33f9ec0b296488c38102191f6d9b`), Step 1 (`child conv-child-first`, lines 2229-2233 and 2278-2310) runs a
synchronous `/bin/sh -c` child that reports `conv-child-first`, prints `CUSTOM-CHILD:conv-child-first`, and exits before
the parent prints `CUSTOM-REPORTED:conv-child-first`. Only after waiting for `CUSTOM-REPORTED:conv-child-first` (when
the pinned child process is already dead) does Step 2 (`parent conv-parent`) run. In
`crates/farhelm-supervisor/src/agent_kind/report_pin.rs:65-77` (`ReportPin::judge`), once the pinned process is dead
(`!alive`) and absent from the incoming ancestry chain, the supervisor accepts the new emitter via dead-pin release
(`PinDecision::Accept`), writing the exact same `acked` hook-log entry, updating `captured_conversation` to
`conv-parent`, and replacing the pin with the parent. Because the test asserts only the hook-log `acked` count and
`snapshot.captured_conversation` (lines 2302-2309) and never asserts the `NotificationKind::CustomPinCorrected`
notification written exclusively on `PinDecision::Correct`
(`crates/farhelm-supervisor/src/service/core/vendor/positional.rs:208-235`), nor keeps the child alive across the
parent's report, the test cannot distinguish ancestor correction (`PinDecision::Correct`) from dead-pin release
(`PinDecision::Accept`). By contrast, `child_first_capture_is_corrected_by_stop_and_resumes_parent` in the same file
(lines 1533-1536) asserts the correction notification (`listed(&h.client, &session.id).await.notifications`). No other
test in the workspace checks `NotificationKind::CustomPinCorrected` or distinguishes custom-agent ancestor correction
from dead-pin release. (Minor secondary note: the docstring also claims 'A child-first report initially resumes the
child', while Step 1 checks only `captured_conversation` rather than `restart_offer`/`resume_argv`, and Step 3 checks
only `refused` count rather than the emitter-pin refusal reason.)

How to verify: Inspect `custom_agent_positional_report_corrects_to_the_parent_and_restarts_it` (lines 2215-2325) against
`ReportPin::judge` (`crates/farhelm-supervisor/src/agent_kind/report_pin.rs:65-77`) and
`Supervisor::report_positional_conversation`
(`crates/farhelm-supervisor/src/service/core/vendor/positional.rs:208-235`).

Fix: Assert that `listed(&h.client, &session.id).await.notifications` is empty after Step 1 and contains one unresolved
`CUSTOM_PIN_CORRECTED_TEXT` notification after Step 2 (and/or keep the first child alive while Step 2 reports so
dead-pin release cannot apply), and optionally check `restart_offer == RestartOffer::Resume` in Step 1 and the
pin-refusal reason in Step 3.

Reviewer metadata (g47_custom_agents_e2e_a/l1): suggested bucket: other (test quality gap in maintainer e2e test suite;
no direct security or user-data-loss route in shipped code, though it leaves custom-agent ancestor-correction
regressions unguarded).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g47_custom_agents_e2e_a/l1`.
