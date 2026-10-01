# Replace with kills a running source session with no warning and no liveness check

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

"Replace with" opens the editable launcher and, when the user presses its "replace" button, creates the new session and
deletes the old one, killing its agent and its terminal tabs if they are running. Unlike plain Replace, nothing tells
the user a running agent or open tabs will be killed, and nothing checks at delete time whether the source is still in
the state it was in when the launcher opened. The launcher can stay open for minutes while the user edits settings. If
the source was restarted in that time (by another client or by an agent), pressing "replace" silently kills the new run.

## Details

Source: whole-codebase review, 2026-09-30, slice ui.

Reviewer's confidence: confirmed (UI side traced; the helm's `ReplaceReq` already has `only_if_nothing_alive` beside
`with`, `crates/farhelm-helm/src/sessions.rs:3018-3025`, so only the UI omits it; the supervisor's unconditional delete
without the flag is taken from the TRIAGE_OUTCOMES.md record cited below).

Reviewer's bucket suggestion: highest.

Possible cover for triage to check: TRIAGE_OUTCOMES.md `## delete-lacks-liveness-precondition.md` added the precondition
only to the unconfirmed delete and to Replace ("Replace sets it when its confirmation showed nothing alive"); Replace
with was not mentioned. SPEC.md "Replace with" (line ~596) says it has "exactly Replace's create-then-delete contract",
and Replace has an inline confirmation that says when the agent will be killed; the spec does not say whether the
launcher's "replace" button counts as that confirmation. Triage should decide which.

- The only destructive cue in the launcher is the button verb: `crates/farhelm-ui/src/list/create_form.rs:2324-2330`
  (`submit_verb = if is_replace_with { "replace" } else { "launch" }`). There is no use of `replace_consequence`,
  `confirm_consequence` or the source's status anywhere in `create_form.rs` or `launch_composer.rs`.
- The request never carries the liveness precondition: `crates/farhelm-ui/src/api.rs:1834-1861` (`replace_session_with`)
  sends `{"intent_key", "with"}` only, and the GitHub-checkout variant sends
  `api::submit_fresh_create(&base, bound.replace_source.as_deref(), ...)` (`create_form.rs:3408`), also without it.
  Contrast plain Replace, `api.rs:2588-2607`, which sends `only_if_nothing_alive` computed from the row the user
  confirmed.
- Call site: `create_form.rs:3437-3450`.

Consequence: the source's agent and tabs are killed, with their in-progress work, without the "confirmation that says so
when anything is still alive" that SPEC.md's Delete rule requires and that plain Replace provides. The wide window (an
open form) makes the stale-source case much more reachable than the timing races the precondition was added for.

How to verify: open "replace with" on an exited session, restart the session through the API, press "replace" in the
launcher; the source's new run is killed and no warning was shown. Also open "replace with" on a running session: the
launcher never says the agent will be killed.

Fix sketch: snapshot `shows_nothing_alive` for the source when the launcher opens (or from the latest listing when
submit is pressed, shown in the launcher) and send `only_if_nothing_alive` on the replace-with request. If the source
has anything alive, show Replace's consequence text in the launcher next to the "replace" button, so the button press is
an informed confirmation.

Related: `header-replace-recomputes-alive.md` and `sidebar-replace-recomputes-alive.md` cover Replace's own YOLO
follow-up dropping the same `only_if_nothing_alive` contract; a shared fix that captures the flag from the prompt the
user answered would serve all three.
