# Introduction promises reboot recovery for unsupported sessions

Reviewed commit: 9f759eb6a62ee475e8a83563e0b6aa7c1e6c00a1

## TLDR

The new introduction promises conversation recovery after reboot for every session, including supported agents that
Farhelm cannot resume.

## Details

Suggested bucket: `highest`. Confidence: confirmed documentation mismatch; user reliance and any vendor-specific manual
recovery were not reproduced. Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: none.

README.md:64–65 and website/src/content/docs/docs/index.mdx:53 introduce the unconditional promise. Cursor, Muse and
OpenCode use Generic integration (crates/farhelm-proto/src/launch.rs:89–101), which has no conversation reporting
(crates/farhelm-supervisor/src/agent_kind/mod.rs:784–793) and always returns NoConversationReporting (:1773–1779).
Restart then refuses rather than recovering the conversation (service/core.rs:3175–3190). SPEC.md:1533–1550 explicitly
says reboot destroys terminal contents and sessions without captured identity offer only Replace. A user relying on this
assurance can reboot ongoing work and discover that Farhelm offers no continuation. Qualify recovery by captured
conversation and supported integration. Similar broad wording already existed in the unchanged artwork; this commit adds
an explicit prose assurance.

Possible cover: SPEC.md, Durability and resume, accepts limited Resume support; it does not justify advertising
universal recovery. No matching queue or ledger coverage found in the searches described below.

Current-main status: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: README.md:64–65 and overview:53 retain the
promise; SPEC.md:230–257 and :1658–1675 retain the exclusions. No fixing commit identified.

Review coverage: full commit diff and relevant code paths read. Runtime behavior and harmful reliance were not
reproduced; the note preserves the reviewer's stated unverified premises.
