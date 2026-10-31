# Retained container startup can reach replacement checkout work

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

Opening a new tab in a retained container can give its old startup state access to a replacement checkout at the saved
path.

## Details

Suggested bucket: **highest**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The retained container's new-tab restart reuses its startup/account state while matching Podman and runc source
establishes fresh binding of the directory currently at the saved mount path. The conditional route requires a stopped
retained container, surviving terminal substrate, usable recorded cwd, a real replacement/re-clone at that path, and
consulted writable startup state outside the replaced bind. Ordinary creation-time account repair does not revalidate
that state on this restart. SPEC.md:1537–1541,3567–3573's tab-path rule and 3676–3689's deliberate-operator exception do
not clearly settle ordinary replacement protection at 4053–4065. Broader ancestor exposure is not required. No actual
replacement, startup payload or work loss was reproduced.

Source locations:
`crates/farhelm-supervisor/src/service/core.rs:13681-13815; crates/farhelm-supervisor/src/container.rs:314-381,403-484; crates/farhelm-supervisor/src/launch.rs:1282-1301`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g112/f4`, `a021/x-g112-f4`, `a046/g112-f4`.
These are original reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and
independent verdicts remain in the private review evidence. No fix or triage decision was executed.
