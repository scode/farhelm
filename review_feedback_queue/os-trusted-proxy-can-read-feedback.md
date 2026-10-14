# An OS-trusted proxy can inspect private feedback

Reviewed commit: 667b4821b933bc61c7bb348df7a9eabf5a2c634e

## TLDR

A proxy trusted by the operating system can read feedback messages and contact information, although Farhelm describes
feedback as private correspondence with its maintainer.

## Details

Suggested bucket: `highest`. Confidence: likely; transport configuration and submitted fields are confirmed statically,
but an installed interception CA and working intercepting path were not reproduced. Source: dedicated `gpt-6.1-sol` high
adversarial review, followed by a fresh independent `gpt-6.1-sol` high drop audit. Proposed drop: the original reviewer
proposed intentional trust-policy coverage; the independent reviewer rejected the absence of explicit specification or
recorded triage acceptance for the confidentiality consequence. Retain for triage.

`Cargo.toml:152–184` adopts reqwest's OS certificate verifier and system-proxy defaults. Production feedback builds that
client (`crates/farhelm-helm/src/feedback.rs:63–72,142–149`) and sends the complete submission (`105–108`). An
intercepting proxy with an OS-trusted certificate can authenticate as the feedback endpoint and read that submission.
This affects feedback from the helm machine, through desktop and web clients. It does not establish disclosure of
unrelated session data or desktop bearer credentials.

This is a deliberate transport-policy choice, not an accidental verification bypass or evidence of an unauthorized CA.
`Cargo.toml:152–158` and `releasing/changelog.d/os-certificate-store.md:5` explicitly intend operation behind proxies
using their own certificates. Historical SPEC.md:1989–2014 describes private maintainer feedback; current
SPEC.md:2149–2174 preserves that description. The independent audit did not find explicit acceptance of third-party
inspection in SPEC.md, SPEC_impl.md, the Planned bucket, BUGS.md, or TRIAGE_OUTCOMES.md. Those intentional comments and
the changelog remain possible cover for the maintainer to assess. Settle the confidentiality policy explicitly before
choosing between documenting this consequence and restricting feedback transport.

Current-main status: persists at `4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703`. Cargo configuration is unchanged;
intervening feedback-file changes concern comments rather than transport. No fixing commit identified.
