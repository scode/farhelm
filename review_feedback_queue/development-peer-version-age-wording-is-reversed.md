# Development-peer age wording has the wrong subject

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The automatic-update specification says development peers have unknown age, while the implementation treats a
development helm as the special case.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

A peer version 0.0.0-unreleased sorts older than a release helm in the inspected comparison. SPEC_impl.md:776–778 and
its nearby comment put the unknown-age explanation on the wrong side. The retained issue is explanatory correctness, not
a request to change the accepted comparison policy.

Source locations: `SPEC_impl.md:776-778`. Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed commit d6645f771bee8ed03a66486ef8755834937d759d; introduced in 04e8b3f97901f79e07e2fc7cb95a211f8cb60335. The
new automatic-scan paragraph describes the host response build-order helpers, then states that development and
unparseable builds retain unknown age. Actual connected-peer eligibility is build_is_older(peer, helm),
automatic.rs:82-84; hosts.rs:354-365 is plain SemVer precedence and gives a development peer 0.0.0-unreleased an older
age than a release helm. The development exception is only in build_is_newer for the helm argument (hosts.rs:372-385),
consistent with existing SPEC_impl.md:5710-5724. Thus this paragraph does not accurately state the automatic target
contract for development hosts. TLDR: the implementation specification suggests a development host has unknown age, but
enabling automatic host updates can select it for replacement with the release helm's build. This is a
specification-description conflict, not a claim that the implementation should preserve development installations or
that a downgrade bypass exists. Existing schema refusal remains in service.rs:1137-1142 and 2019-2029. Suggested bucket
other; confidence confirmed. proposed_drop null. possible_cover: existing dev-helm ordering text and
TRIAGE_OUTCOMES.md:18729-18758 explain intended version behavior but do not repair the new paragraph's development-peer
claim. No established security or user-work-loss consequence.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g138/f2`, `a029/a029-v3`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
