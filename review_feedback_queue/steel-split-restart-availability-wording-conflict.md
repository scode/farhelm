# Steel split-Restart wording conflicts with independent availability

Reviewed commit: 27f5a9690dcc29dee6921c5bbf0ddd06e3e2c8a6

## TLDR

One redesign instruction could disable Restart with when users need to enter a conversation manually, while other
clauses explicitly preserve its independent availability. No current GUI regression is established.

## Details

`plans/queue/steel-session-chrome.md:119–122` applies disabled conditions to both split-button halves. Plain Restart
requires capture, but `SPEC.md:1263–1265` and `crates/farhelm-ui/src/session_view.rs:87–106,126–128,3671–3705` permit
Restart with without capture when a resume command exists. Equivalent shared wording also occurs in foundation, dialogs
and sidebar plans.

Suggested bucket: `other`. Confidence: confirmed wording tension; a regression would require an executor implementing
the coupled reading. Possible cover: the same plan's `5–7,167–172,190–191,448–451` preserves authoritative
specifications, existing per-action availability and approval for weakened guarantees. Later foundation Decisions do not
explicitly alter eligibility, while the specification's broader toolbar summary adds tension. Resolve the contract
without assuming an existing caret bug. Provenance: `g038/f1`, `g034/f2` and neighboring shared-plan observations.
