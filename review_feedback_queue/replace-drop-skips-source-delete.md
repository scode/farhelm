# A dropped Replace leaves both sessions behind

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If the user switches to another session, reloads the page, or the desktop app re-authenticates while a Replace is in
flight, the replacement session is created but the original is never deleted. The user ends up with both sessions, the
old one still running, and no error explaining why.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F3 / COR-REPLACE-DROP`, tagged **definite**. Anchor and title: `crates/farhelm-helm/src/sessions.rs:3485` — a
dropped Replace request creates the replacement but never deletes the original.

Replace (and "replace with") is two steps on the helm: ask the supervisor to create the replacement session (a few
seconds of tmux work, or a git clone for a fresh GitHub checkout), record it, and then send a delete for the source
session (`finish_replacement`, `crates/farhelm-helm/src/sessions.rs:3407`). The HTTP handler for
`POST /api/sessions/{id}/replace` (`sessions.rs:3478-3485`) runs this whole sequence directly on the request's own task.
The helm's web framework drops a handler's task when the client disconnects, which stops it at whatever step it reached.
The helm already has a helper for exactly this, `run_owned` (`crates/farhelm-helm/src/lib.rs:1858`), which moves the
work onto a helm-owned task so a disconnect only loses the reply; host edits were moved onto it in #1196/#1197 (see
`crates/farhelm-helm/src/hosts.rs:654`), but the session routes were not.

So if the client goes away after the create was sent and before the delete, the supervisor finishes the create but the
helm never sends the delete. This is reachable in ordinary use: in the session view, the replace request runs in a task
tied to that view, so switching to another session drops it and the browser aborts the fetch; reloading or closing the
page, or the desktop app re-authenticating and remounting the page, does the same. The user ends up with both sessions,
the source still running, and no error. That contradicts SPEC_impl.md's "Who owns an accepted action" (confirmed
2026-09-28), which says that once the helm accepts a state-changing request, a helm-owned task finishes the work and the
client can only lose the reply. It also contradicts the function's own docs, which promise either a clean replace or an
explicit reply saying both sessions still exist.

The fix is to run the body through `crate::run_owned(async move { do_replace_session(&state, &id, …).await })` with
owned arguments, keeping only the response rendering in the thin handler. Add a regression test in the style of the
existing `owned_work_completes_after_its_waiter_is_dropped`: drop the HTTP future after the fake supervisor acknowledges
the create, and assert that the delete still arrives.

## Review evidence at f087e0b68aed3eb57d90f71b23ef9aa5499cb023

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F9 / COR-REPLACE-OWNERSHIP`, reviewer `correctness_data_flow`, pass 3. Confidence: **definite**. Review disposition:
**would fix**. Queue priority at recording: **high**.

Anchor against the reviewed commit: `crates/farhelm-helm/src/sessions.rs:3494`. Recorded from the completed review
without rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: Same
finding as this existing queue item. `TRIAGE_OUTCOMES.md` already accepts moving Replace onto a helm-owned task, planned
in `plans/triage-restart-takeover-update.md`. That assessment corrects the older report: switching sessions is blocked
by the UI operation lock, and ordinary desktop re-sign-in does not remount the app. It also leaves prompt HTTP-handler
cancellation on client abort unverified. Preserve those caveats; this added review does not supersede the decision.

Replace requires the helm to coordinate two operations: create and accept the replacement, then delete the original. The
HTTP handler performs that sequence directly on the task serving the client connection. If the browser disconnects,
reloads, or times out after the supervisor accepts creation but before the helm sends Delete, cancellation can discard
the remainder of the sequence. The supervisor finishes creating the replacement, but nothing remains responsible for
removing the original. This affects ordinary Replace and Replace with, including fresh-checkout replacements.

Replace can therefore silently become a clone, potentially leaving two agents working in the same directory. Having the
supervisor own each individual operation cannot compensate for the helm never sending the second one. Run the complete
accepted replacement sequence through `run_owned`, the existing helper that keeps helm work alive after its HTTP waiter
disappears. Test cancellation after creation reaches the supervisor but before its reply, and require the original's
deletion to proceed after the HTTP waiter is gone. This follows SPEC_impl.md's accepted-action ownership rule.
