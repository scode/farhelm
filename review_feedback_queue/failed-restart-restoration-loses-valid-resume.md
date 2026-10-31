# Failed Restart restoration can lose the prior Resume offer

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

A named Restart that never starts its replacement can permanently remove the previous Resume offer if the database
restoration write also fails.

## Details

Suggested bucket: **highest**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The new-generation transaction clears capture while retaining its prior value only in memory. A definitive pre-spawn
launch-spec failure invokes abort_relaunch, but a subsequent SQLITE_FULL WAL append failure can leave the committed
clear intact. Once space recovers, the ordinary snapshot reads the cleared record; the settled old report has already
been deleted, and no replacement agent exists to republish it. Bundled SQLite's ENOSPC/append/error paths establish a
compatible conditional sequence. The current regression covers successful restoration, not its write failure. The design
predates the exclusive base. SPEC.md:3437–3447 and SPEC_impl.md:6041–6048's storage-error limits remain ambiguous
against the specific pre-spawn restoration guarantee and prior-valid-offer ledger decision. Independent review retains
highest; no failure was injected.

Source locations: `crates/farhelm-supervisor/src/service/core.rs:12177-12214`. Historical revision qualifiers in those
references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `a038/f13`, `a061/f1`. These are original
reviewer identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain
in the private review evidence. No fix or triage decision was executed.
