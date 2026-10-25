# Devbuild buffers release metadata before authentication

Reviewed commit: be29e118a353b20b8e40331b6abc0a815fb1bcdf

## TLDR

A compromised release origin can provide oversized checksum or signature files that are downloaded and fully read before
rejection. Material resource exhaustion is unmeasured; this is maintainer-tooling resource scope, not a demonstrated
catastrophic failure.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location:
`be29e118a353b20b8e40331b6abc0a815fb1bcdf:scripts/devbuild-mac.sh:594-605; crates/farhelm/src/main.rs:2247-2250`.
Locations and revisions in the assessment below identify the reviewed source; later main is outside this review's pinned
range.

`scripts/devbuild-mac.sh:594-595` downloads checksum and signature metadata with no byte limit, then passes those files
to the verification command at `604-605`. `crates/farhelm/src/main.rs:2247-2250` reads both complete files into memory
before signature verification. A compromised release origin controls those allocations without forging a valid
signature. This is distinct from executable authentication, cache reuse and launching an older staged build.

Confidence: confirmed unbounded ingestion and whole-file allocation by source inspection. Material exhaustion
thresholds, OS response and collateral effects remain unmeasured. The bucket is other for maintainer-tooling resource
scope; no catastrophic failure or loss of unrelated user work is established. Bounding metadata before full allocation
would address this path without requiring a general cryptographic-dependency audit.

Possible cover: no exact cover was found. SPEC.md:3212-3218 requires a size cap for helm payload downloads, whose scope
does not establish acceptance of unbounded devbuild metadata. SPEC.md:3484-3497 and TRIAGE_OUTCOMES.md:7460-7493 accept
host-caused denial of service through terminal streams/logs, a different sender and path. The release-download entry at
ledger lines 3407-3424 concerns operation locking. Existing attachment-cleanup records do not cover metadata ingestion;
the Planned bucket is empty and no matching known bug was found. The rare safe-failure filter does not clearly cover
unauthenticated resource consumption. Proposed drop: none.

Provenance: `a061/f4`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
