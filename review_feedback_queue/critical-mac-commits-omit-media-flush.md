# Critical Mac commits omit the promised media flush

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

On Mac storage with volatile drive caches, a power failure can undo acknowledged credential changes or lose a protected
source identity despite the stronger SQL durability setting.

## Details

Suggested bucket: **highest**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

Bundled SQLite defaults fullfsync off, and the traced Farhelm database setup does not enable it or provide an equivalent
ordered media flush. SQL synchronous FULL/EXTRA is distinct from Mac F_FULLFSYNC. Apple's fsync contract and APFS-era
WWDC19 storage explanation permit drive-cache loss/reordering after ordinary fsync; the pinned SQLite/XNU paths retain
that distinction. This omission predates the reviewed range. The documented volatile-cache case establishes a
conditional route, not loss on every Mac or an absolute guarantee even with F_FULLFSYNC. Specific
critical-identity/revocation promises conflict with broad ordinary WAL-loss acceptance. No power-loss experiment or
hardware prevalence measurement ran.

Source locations: `crates/farhelm-supervisor/src/db.rs:141-188; SPEC_impl.md:4003-4005`. Historical revision qualifiers
in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g177/f5`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.
