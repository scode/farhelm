# Negated isolation probes cannot fail their test

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

The negative loopback probes can pass through a failed denial assertion and still report the test successful.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The Bash negation/errexit context prevents the intended failing probe from enforcing the oracle. This differs from the
separate account-assertion && chain. Production isolation was not tested or shown broken by this oracle defect.

Source locations: `crates/farhelm/tests/e2e/containerized_sessions.rs:698-704`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis preserves f1 at reviewed d6645f771bee8ed03a66486ef8755834937d759d. Local documentation confirmation, without
executing a test: /usr/share/man/man1/bash.1.gz, decompressed source lines 9845-9877 explicitly states set -e does not
exit when the command return value is inverted with !. The complete pinned test function, lines 592-828, was reread. At
698 an unexpectedly successful forbidden connection produces an ignored nonzero negation, immediately replaced by
found=0. At 704 the same ignored status is followed by break, then the independent route-presence test at 708 and
successful witness/readiness printing at 709-710. The final read finish at 711 does not inspect the earlier statuses.
The two servers at 607-608 share a listener that merely writes proxy-ready and stores no accepted-connection count
(listener helper 148-157 read in turn 1). After readiness, Rust checks filesystem canaries, token, conmon
environment/cgroup, file ownership and lifecycle state, then aborts the listeners at 826-827; no later observable
inspects the forbidden connection or resulting bytes. Thus f1 survives complete status-path synthesis. Bucket other;
confidence confirmed; production network bypass not established; proposed_drop none; possible_cover none. Historical
introduced source: 81d6ddbcfaf8ba1bd4e761be9c579e6cea53ea8f, same file:225,231. Initial coverage search remains
applicable. No tests, experiments or remote documentation were used.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g117/f1`, `a004/x4`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
