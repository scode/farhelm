# Restricted connection refusal of whole-environment Delete remains unverified

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

The product requires restricted sessions to be denied this operation. The independent inspection did not establish the
exact denial path from the supplied source excerpts.

## Details

**other.** Unverified after investigation. Source inspection. Main reference
`crates/farhelm-supervisor/src/service/handlers.rs:2999`.

SPEC_impl.md:6224–6227 requires session-authenticated refusal. connection.rs:598 sends an authenticated peer to
handle_restricted_control. Handlers.rs:2999–3000 handles the request only in full dispatch and the restricted fallback
at 3463–3470 refuses it. The existing restricted-request table at 7647–7659 includes DeleteSessionOrSeal and asserts a
matching Unauthorized response. The store transaction separately counts all other durable rows (store.rs:6105–6166),
with the existing exited-row and reopen assertions at 8089–8130. Bucket: other for the checked candidate. Confidence:
pinned source and existing assertions only. proposed_drop: refuted. The restricted route cannot reach the seal handler.
A security consequence would have been highest if a shipped bypass were established. Possible coverage: none.

Latest source check. Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad. TLDR: The environment seal remains
restricted to the full-authority dispatch. Details: SPEC_impl.md:6224–6227 requires session-authenticated refusal.
connection.rs:598 sends an authenticated peer to handle_restricted_control. Handlers.rs:2999–3000 handles the request
only in full dispatch and the restricted fallback at 3463–3470 refuses it. The existing restricted-request table at
7647–7659 includes DeleteSessionOrSeal and asserts a matching Unauthorized response. The store transaction separately
counts all other durable rows (store.rs:6105–6166), with the existing exited-row and reopen assertions at 8089–8130.
Bucket: other for the checked candidate. Confidence: pinned source and existing assertions only. proposed_drop: refuted.
The restricted route cannot reach the seal handler. A security consequence would have been highest if a shipped bypass
were established. Possible coverage: none.

The independent check did not settle the proposed exclusion. AMBIGUOUS via 44137d0 connection.rs:596–608,
handlers.rs:2997–3004, and SPEC_impl.md:6222–6229. Df61991 handlers.rs:2997–3011. The excerpts establish restricted
routing and the separate full-authority handler arm. The restricted match or its DeleteSessionOrSeal Unauthorized
assertion is missing, so the shipped refusal is unverified.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/c26`.
