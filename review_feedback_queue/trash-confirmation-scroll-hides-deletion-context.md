# Confirmation scrolling can conceal destructive-action context

Independent drop decision: a fresh `gpt-6-astra` high reviewer rejected the acceptance exclusion as ambiguous. Bringing
actions into view does not explicitly accept fully concealed target names/totals/permanent-loss context. Suggested
bucket: provisionally `highest`. The user can scroll upward and confirmation remains explicit; this is a possible
uninformed-deletion concern, not a confirmation bypass. Exact browser geometry and accidental deletion were not tested.

Reviewed commit: 7891be3cbf4024dcf39550f30785d637c5e4bb28

## TLDR

A sufficiently tall confirmation opens at its buttons with the selected hosts, totals, or permanent-loss warning above
the visible area.

## Details

Suggested bucket: `highest`. Confidence: likely; concealment follows the layout and scroll target, but accidental
deletion was not demonstrated. Source: dedicated `gpt-6-astra` high adversarial review. Proposed drop: Propose dropping
as accepted presentation behavior under SPEC.md:776–778, which explicitly requires bringing confirmation actions into
view. Independent highest-candidate drop review is required; no drop was finalized..

trash.rs:218–227 scrolls the confirmation's action row into view rather than its explanation. Long host lists exceed the
body; a long unchecked-entry paragraph can also displace the preceding permanent-loss warning entirely. The user can
scroll upward, and explicit confirmation remains required; this is a potential uninformed-deletion concern, not a
confirmation bypass. The comment at 219–221 explicitly describes the warning remaining above the visible actions.

Possible cover: Reviewed SPEC.md:770–778; pinned-main SPEC.md:846–848 preserves the same action-reveal contract. Whether
that contract sufficiently accepts concealed destructive context requires the independent decision.

Status at frozen endpoint: Persists at pinned main: trash.rs:246–255 retains action-row scrolling and explanation above
it. Later affirmative-button focus is outside this commit's scope.

Review coverage: full commit diff read: true. No reviewed-commit diff content unread. An aggregated tool response
truncated historical trash.rs context; missing lines were reopened in full at 390–589. The 13,089-line triage ledger was
searched rather than read exhaustively; unrelated ledger bodies and queue finding files were not read. Backend deletion
implementation was not rereviewed because this commit leaves target construction and dispatch unchanged. Current-main
test additions beyond the inspected excerpt were outside scope. No working-copy queue finding contents were read.

Runtime evidence: None, as requested by default. No checkout files were created, modified, or deleted. Browser-specific
keyboard behavior and extreme viewport geometry remain runtime caveats.
