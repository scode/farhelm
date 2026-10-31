# Account assertion chains can reach false readiness

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A shell helper can continue after a failed intermediate account/home assertion and still publish its ready marker.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The standalone && chain's position makes errexit ineffective for its intermediate commands. Other callers correctly
chain readiness to successful checks, which does not repair the retained sites. This is a test-oracle defect, separately
from the negated isolation probes; no production account failure was reproduced.

Source locations: `crates/farhelm/tests/e2e/containerized_sessions.rs:47-48,698-704`. Historical revision qualifiers in
those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Discovery pass 3 completes an extension of existing f3, with no new finding ID. Reviewed commit
d6645f771bee8ed03a66486ef8755834937d759d. TLDR: individual container fixtures can signal readiness even though
account/home assertions or forbidden-port probes failed. Original assigned introduction remains
f73589c5201a0408f2f1555c18a4efca6f6057be (account helper and standalone isolation caller) plus wrapper caller
2d98a04e3e291bf2b05e9f9bdaf2a9fc2595ee11. Additional observation: the standalone negated timeout/bash probes at :698 and
:704 return a false negated status if the forbidden connection succeeds, but Bash errexit exempts inverted commands. The
script continues; the gateway branch sets found=1 before its probe and unconditionally breaks after it, so found does
not validate refusal. Both paths can reach CONTAINER READY at :710. Primary installed Bash documentation, decoded
/usr/share/man/man1/bash.1.gz:9845-9897, explicitly specifies both nonfinal &&/|| and ! exemptions. Read-only git log -S
metadata identifies the negated probes' initial provenance as 81d6ddbcfaf8ba1bd4e761be9c579e6cea53ea8f; they predate the
assigned account/wrapper patches and are supporting context, not newly introduced by them. Suggested bucket other;
confidence confirmed source-level test-oracle gap, no experiment or observed false pass. No production account failure,
host-loopback reachability, data loss or security consequence is inferred. The account-rule/direct-exec/tab
marker-in-chain exclusions remain unchanged. proposed_drop none. possible_cover none found: required searches checked
specs, empty Planned/queue, BUGS, queue INDEX and ledger for gateway, loopback, negation, network assertions and
errexit. SPEC.md:3622-3627 and SPEC_impl.md:5807-5808 state the selected-port contract; landed podman-sandbox
plan:165-171 demands real tests for other-port refusal, not acceptance of this assertion omission. Historical plan
runtime observations are not independently authenticated evidence. This is the same Bash assertion-checking class
retained under f3 rather than a rephrased new finding.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g126/f3`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
