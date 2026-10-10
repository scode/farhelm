# Restart with can execute after Cancel

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A stale Restart with submission could execute after Cancel.

## Details

F55 — **possible** — `crates/farhelm-ui/src/restart_with.rs:389` — Restart with can execute after Cancel

Cancel clears the dialog and releases its operation claim, but the ordinary submission handler does not require either
to remain live. If a captured submission is delivered before unmount invalidates it, a valid changed draft could still
restart the session, including stopping a working agent when captured consent permits it. That event ordering has not
been reproduced, and launch/resume checks still apply. Bind every submission to a live opening and claim invalidated
synchronously by Cancel, preserving existing authorization checks.

## Evidence and triage context

- crates/farhelm-ui/src/restart_with.rs:385-395: primary checks captured busy plus current draft validity/change, then
  calls on_submit; it does not establish that the dialog remains open.
- crates/farhelm-ui/src/restart_with.rs:709-710: the ordinary action calls primary with both approval flags false.
- crates/farhelm-ui/src/session_view.rs:2421-2433: Cancel clears restart_with_open, the YOLO question, and view_claim
  without setting restarting.
- crates/farhelm-ui/src/session_view.rs:2448-2499: ordinary submission checks restarting, current availability,
  unchanged baseline launch, and matching launch kind/type, but neither restart_with_open nor view_claim. It forwards
  the captured restart_with_stops_first.
- crates/farhelm-ui/src/session_view.rs:1298-1329: the shared restart closure checks only restarting before issuing
  restart_session.
- crates/farhelm-helm/src/sessions.rs:2324-2333: valid edits pass the separate YOLO policy check and forward
  stop_if_running to the supervisor.
- crates/farhelm-supervisor/src/service/core.rs:10244-10253: a working agent is refused only when stop_if_running is
  false; otherwise stop_live_agent runs.
- e2e/tests/restart-with.spec.ts:433-446: the adjacent YOLO test deliberately delivers Cancel followed by an enabled
  answer in one synchronous burst. This supports the event-order hypothesis but does not reproduce the ordinary-action
  case.
- crates/farhelm-ui/src/restart_with.rs:703-710: the ordinary button calls primary with (false, false). At :385-395,
  primary checks the captured busy value and live draft validity, but no live opening or operation claim.
- crates/farhelm-ui/src/session_view.rs:2421-2433: Cancel clears restart_with_open, clears the YOLO question, and
  releases view_claim. It does not set restarting.
- crates/farhelm-ui/src/session_view.rs:2448-2499: the submit handler checks restarting; allow_yolo=false bypasses
  take_restart_yolo and reaches with_restart when the session and edit remain valid.
- crates/farhelm-ui/src/session_view.rs:1293-1329: the shared restart closure sets restarting and spawns restart_session
  without checking the cancelled opening or reacquiring its claim. At :2494-2499 it receives the rendered stop-first
  consent.
- crates/farhelm-ui/src/api.rs:2150-2157 and crates/farhelm-helm/src/sessions.rs:2287-2298,2331-2333: the request
  forwards stop_if_running to the supervisor.
- crates/farhelm-supervisor/src/service/core.rs:10244-10253: true stop_if_running bypasses the working-agent refusal and
  reaches stop_live_agent.
- e2e/tests/restart-with.spec.ts:433-446: existing coverage explicitly delivers Cancel followed by a still-connected
  affirmative button in one synchronous burst, establishing the event-order premise for the related YOLO path.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- {"basis": "TRIAGE_OUTCOMES.md:6887-6902, yolo-restart-cancel.md", "comparison": "Shares Cancel-before-answer timing
  and potential process termination, but explicitly addresses the two YOLO answers and their live-question protection.
  Ordinary submission carries neither answer."}
- {"basis": "TRIAGE_OUTCOMES.md:6904-6920, restart-parent-cancel.md", "comparison": "The recorded trigger is accepting
  allow_yolo without a live question; completion requires consuming that question for approval-bearing requests. This
  finding follows the allow_yolo=false branch."}
- {"basis": "review_feedback_queue/FILTER.md:119-135", "comparison": "The timing may be sub-second, but stopping a
  cancelled action's process tree is outside the filter's recoverable-consequence boundary."}
- TRIAGE_OUTCOMES.md:6887-6902, yolo-restart-cancel.md: covers cancelled YOLO answers, including unrestricted restart
  and permanent approval.
- TRIAGE_OUTCOMES.md:6904-6920, restart-parent-cancel.md: specifically requires consuming a live question for
  approval-bearing restart. Ordinary submissions carry no such approval and bypass that check.
- review_feedback_queue/FILTER.md:116-135: the sub-second race filter excludes loss of processes or other user-owned
  work.

Caveats:

- No runtime reproduction was performed or authorized.
- Requires the stale ordinary action to reach its handler before unmount invalidates it.
- A valid changed draft, unchanged stored launch, and available Resume offer are also required.
- This does not bypass the helm's separate YOLO policy.
- SPEC.md:757-782 accepts unconfirmed restart of idle/waiting/unknown agents; it does not authorize a cancelled restart.
- No runtime reproduction was performed. Confirmation is by tracing a valid changed draft and Cancel-then-submit
  delivery before rerender.
- The supervisor still validates the launch and resume offer. The trigger requires those checks to pass.
- This does not bypass the separate YOLO authorization check.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_14_sec:p1:F1`, `ui_desktop_14_cor:p1:F1`.

- `ui_desktop_14_sec:p1:F1`: confidence as filed: possible; suggested bucket as filed: highest.
- `ui_desktop_14_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
