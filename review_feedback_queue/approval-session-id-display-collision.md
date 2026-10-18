# Approval cards can show different session IDs identically

Reviewed commit: 5e1f1cfffe38f41e450ab70d440ca8fabac8be36

## TLDR

An approval card can show the same session identifier for two different sessions reported by a dishonest or broken host.
This obscures which session is asking or being acted on. A mistaken approval or resulting loss has not been
demonstrated.

## Details

The requesting session and lifecycle targets use `session_rows` (`crates/farhelm-ui/src/approvals.rs:394–400,721–778`).
Its session ID becomes `CardRow::Value` at :1023, whose renderer calls `display_peer` at :506–510. That renderer escapes
unsafe characters but leaves literal escape notation unchanged (`peer.rs:64–86`). For example, the literal ASCII ID
`peer<U+200B>` and the distinct ID consisting of `peer` followed by U+200B both produce exactly `peer<U+200B>`. This
collision follows from the string transformation and does not depend on font or browser layout. U+200B is in the unsafe
set (`crates/farhelm-proto/src/text.rs:68`); it is not a control character rejected by
`crates/farhelm-helm/src/session_cache.rs:56–68`. Both bounded nonempty IDs pass that ingress check.

Ordinary ASCII spaces are another distinction the renderer preserves raw. The relevant CSS isolates direction and wraps
values but does not preserve whitespace (`crates/farhelm-ui/assets/app.css:4586–4588,7150–7172`). The resulting
leading/trailing-space ambiguity was not rendered in a browser during this review.

Use `CardRow::Identity` for session IDs. It already invokes `display_identity` (:528–531), whose injective spelling
escapes spaces, literal escape markers and placeholder delimiters (`peer.rs:124–140`). A focused verification would
compare both requesting and target ID rows for literal escape notation, the corresponding format characters, and space
variants, while checking that dispatch still uses the original ID. No runtime checks were run.

Suggested bucket from the producer: **other**, with **possibly highest** if the display changes an authorization
decision or causes work loss. Independent disposition: **KEEP, provisionally highest**, because this is an identity
disclosure used for authorization and the security consequence remains unresolved. Confidence: **confirmed static
renderer/ingress mismatch**; mistaken approval, browser whitespace behavior, cross-host bypass and data loss are
unverified. Shipped supervisors mint UUIDs and are unaffected. Cards separately disclose title and host, and the action
still dispatches by its exact raw ID. These facts limit the claim; this is not evidence of a routing bypass.

Possible cover: `SPEC.md:2837–2841` requires compared identity labels to remain distinguishable. The decisions
`TRIAGE_OUTCOMES.md:8545–8562`, `ascii-spaces-remain-invisible-host-identity-approval-labels.md`, and `:13663–13676`,
`installation-identity-escape-collision.md`, fixed installation labels through `display_identity`. They neither fix this
remaining session-ID caller nor accept its ambiguity. The queue's accessible-title finding concerns a different renderer
and surface. No exact Planned, BUGS or queue coverage was found.

Proposed drop from the producer: conditional application of FILTER.md, “Rare, self-correcting glitches and imprecise
diagnostics,” if the entire consequence is only an imprecise diagnostic from inputs shipped peers never send.
Independent drop-review-5 rejects that proposal: the filter expressly excludes security and wrong-target state changes,
and the uncertainty about this authorization disclosure prevents a full match. No severity escalation beyond that
uncertainty is claimed.

Provenance: persistent Astra/high review, batch `ui-session-detail-1`, finding `f4`; both f4 events in
`groups/ui-session-detail-1/findings.jsonl` (lines 7–8), final synthesis `turn2.result.md`, finding f4. Independent
adjudication: `drop-review-5/verdict-addendum.md`, item 2.
