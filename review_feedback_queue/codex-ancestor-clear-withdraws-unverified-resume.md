# An unverified ancestor clear can withdraw the current Resume offer

Reviewed commit: 4423eda1a10b0bf7d7a418efc08778b3992a87d3

## TLDR

After child-first capture, an ancestor’s clear report can remove a working Restart target before Farhelm verifies the
replacement conversation.

## Details

Suggested bucket: `highest`. Confidence: likely; the control-flow exception is confirmed, but a real Codex ancestor
emitting clear before its transcript becomes available was not reproduced. Source: dedicated `gpt-6.1-sol` high
adversarial review. Proposed drop: none.

At crates/farhelm-supervisor/src/service/core/vendor/codex.rs:176–239, correction bypasses the prior binding’s
constraints, but the source == "clear" exception still admits a nonresumable locator. CodexLocator::verify at
agent_kind/codex.rs:204–214 returns successfully without establishing readiness when the path or complete header is
absent. Admission then replaces the conversation and pin and records the correction notification at
vendor/codex.rs:254–306 and store.rs:4989–5001. Trigger: a child’s verified report wins first; its recorded ancestor
emits SessionStart/clear before its new transcript is available. Consequence: the child’s existing Resume offer
disappears, and authority moves to the ancestor without verified root metadata; if the ancestor never persists that
conversation, ordinary Restart cannot recover the previous offer. SPEC.md:1592–1595 and SPEC_impl.md:1982–1987
explicitly require unverified ancestor reports to change nothing. Require resumability for correction before applying
the ordinary pending-clear exception. The new correction test covers malformed selecting records and missing Stop
records, but not missing clear records (core.rs:19633–19672).

Possible cover: SPEC.md:1602–1604 and SPEC_impl.md:2000–2001 permit ordinary foreground clear reports to install pending
locators. Whether that exception intentionally extends to ancestor correction needs clarification; it conflicts with the
explicit unverified-ancestor rule.

Current-main status: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703. The admission, locator, pin and store files
are unchanged since the reviewed commit; current SPEC.md:1695–1701 and SPEC_impl.md:2047–2052 retain the
verified-ancestor requirement. No fixing commit found.

Review coverage: the full commit diff and relevant failure paths were read. No product runtime tests were run; unchanged
surrounding code was inspected only where relevant.
