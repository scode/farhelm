# First-contact retry keeps an unidentified row snapshot

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

After recording a host's first identity but failing to finish connecting, an automatic retry can accept a peer that
omits its identity. The specification requires refusal at that point. Shipped supervisors cannot produce the required
identity omission, and no added security access or lost work is established.

## Details

`crates/farhelm-helm/src/manager.rs:3298–3306` reads the host row before the retry ladder. Each attempt reuses that
snapshot (`3480–3565`). First contact commits an identity (`3919–3932`; `store.rs:6639–6675`) before awaiting browser
forwarding setup (`manager.rs:3676`). A setup failure returns an ordinary retryable failure. If the next hello omits
identity, the stale row still says None (`3857`), so the attempt accepts the peer (`3900–3902`) although the database
already records an identity.

`SPEC_impl.md:3750–3759` requires a recorded row meeting an identity-less peer to freeze in identity-unverified and
connect nothing. The retry must consult the current identity or otherwise retain the first-contact transition if that
contract is to hold for custom protocol peers.

This is not an ordinary shipped-supervisor failure. Owned construction obtains `Some(identity)`
(`farhelm-supervisor/src/service/core.rs:5588–5592`); serving requires ownership (`6997–7007`), and every hello sends
the immutable construction-time value (`service/connection.rs:183–191`). A custom peer can send the sequence, but it
could also repeat the identity it originally supplied. This finding establishes no new impersonation authority,
cross-entry routing, secret access, or unauthorized operation.

Suggested bucket: **other**. Confidence: **confirmed conditional control flow and refuted shipped identity-less trigger;
custom-peer practical impact unverified**. The original potential-highest interpretation received independent review;
that review rejected dropping the whole contract discrepancy while narrowing its consequence. No runtime work was
performed, and reproduction requests from the initial pass are superseded by the user's source-only instruction.

Possible cover: ledger `update-identity-none-plan-confirm-disagree.md` (`TRIAGE_OUTCOMES.md:4134–4147`),
`hostnotfound-refresh-keeps-serving.md` (`12342–12351`) and `tunnel-never-pins-learned-identity.md` (`16123–16132`)
discard other identity-less cases without shipped triggers. They may inform triage but do not exactly match this retry
transition. The never-identified-peer allowance (`SPEC.md:834–840`) does not cover a committed identity. No exact
Planned, BUGS or queue item was found. Proposed drop: rejected by the independent assessment; retain narrowly.

Provenance: persistent reviewer `A18-first-contact/A18-F1`, independent `drop-check-09`, parent source and coverage
verification.
