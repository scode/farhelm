# Antigravity tests miss pin changes on the first unverified report

Reviewed commit: c0835826d469bf115dff16c68da784ab8750cf85

## TLDR

The Antigravity tests can pass despite recording a process identity from a rejected first callback.

## Details

**other**. Confirmed by source and contract inspection.

the source refutes an existing state mutation on rejection (service/core/vendor/positional.rs:187–204,234–247 and
store.rs:5643–5658). A targeted regression assertion would check both durable pin and identity before the first verified
report. Consequence narrowed in synthesis: a rejected same-process child and a later valid main callback share a
process, so prematurely pinning that emitter alone would not block the later main callback. An incorrectly retained pin
for a different emitter is the authority concern. Existing process ancestry and correction policy still apply.
SPEC.md:3942–3947 and SPEC_impl.md:4495–4502 require rejection before any identity or pin change. Confirmed coverage
gap, estimated other, possible_cover none and proposed_drop none. The pinned tip does not change these tests.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `antigravity/f4`.
