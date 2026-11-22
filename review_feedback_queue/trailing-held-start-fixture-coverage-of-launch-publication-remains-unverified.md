# Held-start fixture coverage of launch publication remains unverified

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

The fixture checks publication after first-stage entry. Its coverage of the ordering relied on by startup recovery
remains unverified.

## Details

**other.** Unverified after investigation. Source inspection. Main reference
`crates/farhelm-supervisor/src/service/core.rs:24642`.

core.rs:24642–24654 describes publication before any host start. The observer waits until the fake host has published
its entered marker and then reads the row (24791–24818). The consumed-release marker and startup-error assertions
(24856–24870) independently prove the first stage remained in progress across the row read. They rule out a write
delayed until start completes, as the comment's example says, but do not rule out a future write after first-stage
invocation and before the observation. The intended boundary matters because a running agent may report immediately.
Either describe the narrower test oracle or assert the durable row at a seam ordered before first-stage launch. Bucket:
other (regression-test weakness). Confidence: confirmed by source inspection of the existing assertions. No regression
observed. Possible coverage: lifecycle_unit interface if it carries a stronger launch-ownership assertion.

Latest source check. Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad. TLDR: The existing test observes durable
provenance after entry to the fake first stage and before it returns. Details: the entry marker precedes the read at
core.rs:24791–24818 and the consumed-release witness at 24856–24870 confirms the read happened before the first stage
returned. This does not independently detect a write between first-stage entry and the read. The #4529 hunks at
24565–24581,24598–24601 and 24835–24836 only repair statements about releasing the held host, bounded acknowledgement
and cancellation ownership. The test's ordering assertion and claim at 24642–24654 are unchanged and depend on a
separate launch contract. Bucket: other. Confidence: confirmed by reading existing assertions. proposed_drop: set aside
the additional ordering oracle for this review, since the changed cleanup contract neither claims nor depends on that
stronger oracle. Could not be highest. Possible coverage: lifecycle_unit interface. C17 in the same test.

The independent check did not settle the proposed exclusion. AMBIGUOUS via 44137d0 core.rs:24576–24583, 24640–24657, and
24789–24818. The row is read after the fake signals first-stage entry, so the excerpt supports the narrower oracle. The
complete cleanup changes and release-consumption assertion are missing, leaving the proposed scope exclusion unverified.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/f12`.
