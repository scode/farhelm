# Delete-level tests miss the retained Gone host refusal

Reviewed commit: 063203912059cb23101d9a458d8821f02f1bc18f

## TLDR

The existing assertions do not show that Delete keeps a session and its launch record if a terminal classified Gone
still has a live adopted process.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core/tests/host_terminals.rs:4267`.

core/tests/host_terminals.rs:4267-4292 asserts Gone, then the shared end_launch_host refusal and the public CloseTab
refusal, retained tab and retry. :1155-1197 asserts public Delete refusal and retention with an Unavailable terminal.
:1385-1430 holds the Delete end gate before dispatch and :1445-1468 uses a session with no held terminal and a genuinely
dead recorded socket. session_hosts.rs:238-260,708-739 is the Delete-specific bridge from held runtime to shared
teardown, and SPEC_impl.md:2162-2172 requires keeping the session and records on unconfirmed end. An assertion
exercising unconditional Delete with a held Gone main or tab runtime and verifiably live adopted peer would pin that
bridge and its retention result. The required assertion is absent from the checked assigned tests through
df6199142d6495a95b2d008400ba2686efb775c6. Bucket: other, assertion gap only. Source still routes the held host
correctly. Confidence: source and existing assertions, no observed run. Possible coverage: lifecycle_unit and
lifecycle_e2e. Distinct from the recorded-scope enumeration assertions, guarded consent failures, and r1's shutdown
reconnect branch. teardown.rs:541-560 confirms that an end error returns before attachment or row removal. The coverage
claim is limited to the missing assertion on that behavior, not a current Delete failure.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery/r15`.
