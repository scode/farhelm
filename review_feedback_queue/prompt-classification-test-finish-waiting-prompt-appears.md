# Prompt-classification test can finish waiting before its prompt appears

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A delayed prompt could make its classification test fail on correct behavior.

## Details

F168 — **possible** — `crates/farhelm-supervisor/src/service/ticker.rs:5971` — Prompt-classification test can finish
waiting before its prompt appears

The fixture's readiness predicate establishes a quiet streak, not that either pane has printed its expected dialog. If
shell startup is delayed while sampling continues, blank captures qualify and the classifier correctly reports generic
Idle instead of Waiting. No failure was observed. Establish expected dialog content in both panes before evaluating
quietness, then retain the independent Waiting and Idle assertions.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/ticker.rs:1970–1981: spawn_pane proves pane existence without observing output.
- crates/farhelm-supervisor/src/service/ticker.rs:2039–2048: the session entry is installed immediately afterward.
- crates/farhelm-supervisor/src/service/ticker.rs:5957–5988: the test waits for unchanged_streak >= 3, stops sampling,
  then requires Waiting.
- crates/farhelm-supervisor/src/service/ticker.rs:545–563: repeated identical blank captures increment that streak.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- {"matched": null, "possible": "SPEC_impl.md:1661–1674 specifies anchored prompt recognition and generic fallback for
  unrecognized content. It supports the conditional Idle result; it does not establish fixture readiness."}

Caveats:

- No failing run was observed.
- The open premise is shell startup remaining delayed through the qualifying samples.
- This is independently editable from the busy-pane test at ticker.rs:5869.
- The material premise is shell startup remaining delayed through the qualifying captures.
- Independent assertion site from ticker.rs:5869.
- No runtime tests were run.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_08:p1:F3`.

- `gap_supervisor_state_cor_08:p1:F3`: confidence as filed: possible; suggested bucket as filed: other.
