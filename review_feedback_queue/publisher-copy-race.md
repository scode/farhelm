# Screenshot publication may certify pixels different from those it checked

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Screenshot publication may certify pixels different from those it checked.

## Details

`F49 / COR-PUBLISHER-COPY-RACE` — **possible** — `scripts/publish-docs-shots.sh:253` — Screenshot publication may
certify pixels different from those it checked

The screenshot publisher verifies each local PNG against the complete capture's checksum record. It then creates its
temporary repository, performs remote reads, and only afterward copies the PNGs into the snapshot. Those source files
remain mutable in the meantime. The copied files are checked for PNG format and minimum size, but are not compared again
with the capture checksums.

If another capture changes those files between verification and copying, the snapshot could contain different pixels, or
pixels from multiple captures, while its manifest names the earlier capture's source commit and time. That concurrent
same-checkout capture was not observed. Copy into private staging first and verify those copies against the captured
record before pushing, or serialize capture and publication with a shared lock. Proposed bucket: other.

Possible cover: root `AGENTS.md:647–654` requires one agent per checkout, and the screenshot SPEC prescribes a
sequential capture, inspection, and publication workflow. Those rules may fully exclude the proposed overlap.

Restater note: The source has the verification-to-copy gap, but the coordinator should decide whether the supported
concurrency contract already covers it. This is conditional on overlapping mutation of the same checkout's capture
files.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **other**.

Originating reviewers and passes: `auto_data p1`, `auto_trust p2`.

Possible cover recorded during collection: RootAGENTS647–654 oneagentpercheckout plusmanualsequencing..

Collection caveats: Concurrent samecheckoutcapture unobserved; oneagentpercheckout root647–654 mayfullycover,
needscoordinator verdict.

Coordinator confirmation: independent audit p4 rejected a blanket one-agent-per-checkout exclusion because a single
operator can overlap commands. The overlap remains unverified.

## Filed reviewer metadata

- `auto_data p1`: confidence as filed: possible / likely; open premise is a capture modifying the same checkout while
  publish runs. Suggested bucket as filed: other. Confidence: possible / likely; open premise is a capture modifying the
  same checkout while publish runs.
- `auto_trust p2`: confidence as filed: possible; the material premise is a same-checkout capture or edit replacing a
  PNG after verification and before the publisher copies it. The identity gap is source-grounded, but this interleaving
  was not run. Suggested bucket as filed: other, successful publication with incorrect durable provenance or mixed
  captures.
