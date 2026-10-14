# Composing Escape can discard the save-template draft

Independent `gpt-6-astra` high exclusion audit rejected the proposed pre-existing-only drop. The composing Escape scope
has no exact existing coverage; the Enter-only remediation does not settle it. Retained for human triage.

Reviewed commit: 7522d172fce0699cd5e85547a0b536eb821bc13d

## TLDR

Cancelling an input-method candidate can close the save-template panel and lose its unsaved name and checkbox choices.

## Details

Suggested bucket: `highest`. Confidence: likely: loss of the component-local draft follows from the code; delivery of
composing Escape by a supported native IME was not reproduced. Highest is conservative because unsaved user edits are
involved.. Source: dedicated `gpt-6-astra` high adversarial review. Proposed drop: Propose excluding from this commit’s
actionable results as a pre-existing adjacent defect, unchanged by its Enter-only fix. Independent drop review is
required; no independent reviewer was spawned under the task’s prohibition..

crates/farhelm-ui/src/list/save_template.rs:197–200 handles every Escape as cancellation without checking composition.
Its name and checklist are component-local signals at 153–154; create_form.rs:4726–4730 removes the component on
cancellation. An IME-delivered Escape with is_composing=true therefore discards those edits instead of merely dismissing
the candidate. The launcher’s underlying session choices remain. Rename already distinguishes composing Escape at
rename.rs:251–259. This handler is unchanged from the first parent; the reviewed change at save_template.rs:210
addresses Enter only.

Possible cover: SPEC.md:591 says Escape returns to the launcher, but does not explicitly address IME candidate
cancellation. TRIAGE_OUTCOMES.md heading ime-confirmation-save-template-prematurely.md explicitly scopes its completion
criterion to composing Enter.

Status at frozen endpoint: Still present at pinned main: save_template.rs:199–202, with the unmounting callback at
create_form.rs:5022–5025. No fixing commit identified.

Review coverage: full commit diff read: true. No commit-diff content remains unread. Surrounding modules and
specifications were read selectively, not in full. The rest of the 13,089-line triage ledger was searched rather than
read sequentially. Current queue finding bodies unrelated to this commit were not opened. Minified xterm was inspected
through bounded relevant excerpts, not as a complete file. Two oversized combined outputs were truncated; affected diff
content was reread, and specification conclusions rely on subsequent explicit numbered ranges rather than truncated
search output. No Sol reports or working-copy queue additions were read.

Runtime evidence: None run; findings are static review. Native IME delivery and the mouse-reporting browser gesture
remain unverified at runtime. No files were created, modified or deleted; no fetch, rebase or helper agents.
