# Update reminders precede the initial provisioning read

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

An Update reminder can advertise availability before the first provisioning-state read has established whether that
action is eligible.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The inspected initial publication path precedes the eligibility read. This is a presentation/admission-order
inconsistency; the later operation still checks its actual admission. No wrong-target update, unintended authority or
user-work loss was established.

Source locations: `crates/farhelm-ui/src/hosts/updates.rs:49-69; crates/farhelm-ui/src/provisioning.rs:2923,3754-3781`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Reviewed full commit d6645f771bee8ed03a66486ef8755834937d759d; integration b2a3c6162db244516f1b2ee23338622de0f338d4.
Parent read/audited discovery3 and accepts f3 as an Other early-loading Update offer defect. This retains the full
source chain and evidence from f3-final: an empty initial progress snapshot publishes run_active=false and
menu.update=true independently of its asynchronous first HTTP read; the reminder interprets present menu state as loaded
availability. SPEC.md:1838-1839,1869-1870 and SPEC_impl.md:754-759 exclude not-yet-loaded options. No high/highest route
is established because backend Busy preserves concurrent-run safety. Confidence confirmed source-only omission; no
timing measurement or runtime reproduction. proposed_drop none. possible_cover correction: the rare-glitch filter is not
an exact obvious match; ordinary initial mount needs no extra rare condition, per parent's accepted assessment. Browser
fixtures only prove post-read routing and planning-refusal behavior, not successful installation. Original events and
the prior filter caveat remain in the append-only history. This continuation begins another discovery pass beyond f3 and
the independently excluded f1/f2 cases.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g135/f3`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
