# Replace omits the conversation-loss warning after a failed resume

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Replace omits a conversation-loss warning after a failed resume launch.

## Details

F59 — **definite** — `crates/farhelm-ui/src/status.rs:538` — Replace omits the conversation-loss warning after a failed
resume

A failed final agent launch leaves an Error session that can still retain its conversation and offer Resume. Replace
nevertheless describes it as an agent that never started and omits the conversation-discard warning before deleting the
original session. Confirmation remains required, but its account of recoverable state is wrong. Keep that warning for
Error sessions unless actual conversation evidence shows nothing resumable exists. Deletion of an external transcript is
not established.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/core.rs:10403-10419: restart claims a new generation using the captured
  conversation and ownership version.
- crates/farhelm-supervisor/src/store.rs:4073-4095: the relaunch UPDATE resets outcome, pane and launch provenance while
  leaving conversation-capture columns intact.
- crates/farhelm-supervisor/src/service/core.rs:2882-2897: the relaunched in-memory entry clones the previous capture
  into its new run state.
- crates/farhelm-supervisor/src/launch.rs:949-956: failure to exec the resume program writes an exec_failed launch
  sentinel.
- crates/farhelm-supervisor/src/store.rs:457-467: SentinelError converts Launching, Running and other eligible outcomes
  to Error without clearing conversation capture.
- crates/farhelm-supervisor/src/service/status.rs:372-377,411-412,459-476: the restart offer is computed from committed
  capture separately from status and launch-error reporting.
- crates/farhelm-supervisor/src/agent_kind/mod.rs:1773-1807: Resume eligibility checks integration, template and
  captured identity/provenance, not the current run's Error status or executable availability.
- crates/farhelm-ui/src/status.rs:440-446,524-538: Replace adds its fresh-successor clause, but the Error branch says
  only 'the agent never started', unlike the other branches' conversation-discard warnings.
- crates/farhelm-ui/src/session_view.rs:2289-2312 and crates/farhelm-ui/src/list/row.rs:1954-1985: real header and
  sidebar confirmations use this wording and dispatch replacement on confirmation.
- crates/farhelm-helm/src/sessions.rs:3128-3145: successful replacement proceeds to delete_session_with for the original
  session.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- {"basis": "TRIAGE_OUTCOMES.md:877-893, failed-restart-discards-capture.md", "comparison": "That decision concerns
  capture already judged non-resumable before a failed restart. Lines 886-889 explicitly preserve the distinction for a
  valid Resume offer. Here capture survives, Resume can remain available, and a later Replace removes the association
  under misleading wording."}
- {"basis": "SPEC.md:798-805 and SPEC_impl.md:2654-2672", "comparison": "These specify that Replace creates a fresh
  successor and deletes the source. They establish the destructive consequence; they do not accept inferring that an
  errored session never had a conversation."}

Caveats:

- Replace still requires confirmation and describes a fresh successor. This is misleading or incomplete destructive
  wording, not an entirely unconfirmed replacement.
- The established loss is the original Farhelm session and its resume association; deletion of the harness's external
  transcript is not established.
- The concrete trigger is an accepted resume launch whose final agent exec fails, rather than a prelaunch refusal that
  restores the prior outcome.
- No runtime reproduction ran.
- TODO.md:33-43, BUGS.md and the current queue index contain no matching coverage. The status-history filter does not
  cover losing a valid Resume association; review_feedback_queue/FILTER.md:86-94 excludes that consequence.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_15_sec:p1:F2`.

- `ui_desktop_15_sec:p1:F2`: confidence as filed: definite; suggested bucket as filed: highest.
