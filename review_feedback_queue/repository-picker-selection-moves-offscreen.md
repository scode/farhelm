# Repository keyboard navigation leaves the selection offscreen

Reviewed commit: dccdb7449698c59175ec8d90d1dae1f881f4494d

## TLDR

When repository suggestions overflow, keyboard users cannot see which repository Enter will select.

## Details

Suggested bucket: `other`. Confidence: confirmed. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed
drop: none.

At crates/farhelm-ui/src/list/create_form.rs:5315–5318, ArrowDown/ArrowUp update repository_index without scrolling the
active option. Focus remains on the input; options have tabindex=-1 at 5335–5336. assets/app.css:5310–5314 caps the list
at 180px with overflow:auto. With enough suggestions, navigating beyond the visible rows—or pressing ArrowUp to wrap to
the last row—leaves the selected repository hidden. Add a scroll-to-active-option action, as the existing model picker
does at launch_controls.rs:259–271 and 475–482. Scope: repository selection, before preview and launch; no automatic
launch or demonstrated data loss.

Possible cover: No matching specification acceptance, Planned item, BUGS entry, queue item, ledger decision or filter
found. The new test at e2e/tests/github-checkout-composer.spec.ts:124–186 supplies only one suggestion.

Status at frozen endpoint: Persists at pinned main 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: create_form.rs:5653–5656
still updates only the index; app.css:5655–5659 retains the scrolling container. No fixing commit identified.

Review coverage: full commit diff read: true. No commit diff hunks remain unread. Unchanged surrounding modules and
tests were read selectively, not exhaustively. TRIAGE_OUTCOMES.md was searched throughout and relevant excerpts read;
its entire 13,089-line body was not read line by line. Unrelated queue files were screened through INDEX.md. Oversized
initial search/diff outputs were rerun in smaller selections before reliance. No checkout files were created, modified
or deleted.

Runtime evidence: None, as requested by default. Findings are based on source inspection; browser scrolling and the
template presentation issue were not reproduced. The supplied full test-authoring checklist was applied to the changed
tests and relevant fixtures.
