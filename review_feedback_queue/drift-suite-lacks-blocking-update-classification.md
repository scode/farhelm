# The drift runner has no unavailable verdict for a blocking Codex update

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The drift guide calls for an unavailable verdict on a blocking CLI update, but the suite has no update-specific
classification. The exact verdict for the documented Codex screen has not been established.

## Details

The installed-agent guide lists a blocking CLI update as an unavailable prerequisite
(`docs/harness-drift-tests.md:27–40`). The suite’s readiness classifier has no dedicated update verdict
(`crates/farhelm/tests/e2e/real_agent_capture/drift.rs:250–301,450–500`). A separate Codex audit documents a blocking
update screen (`crates/farhelm/tests/e2e/real_agent_capture.rs:1307–1315`). That audit does not establish whether the
drift suite reaches or classifies that exact screen, so a specific false FAILED verdict remains unverified. Define the
supported blocking-update signal and classify it consistently with the guide.

Confidence: likely when the documented Codex update screen blocks the configured launch and does not contain a
recognized credential marker. Source confirms there is no update-specific verdict. The exact screen text and timeout
stage were not available in the pinned captures.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `driftRunner/l13`.
