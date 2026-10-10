# The upload-memory probe assumes Linux and 4 KiB pages — RSS procfs observation (macOS portability)

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The upload-memory test cannot measure RSS on native macOS.

## Details

F151 — **definite** — `crates/farhelm-supervisor/src/files.rs:1537` — The upload-memory probe assumes Linux and 4 KiB
pages — RSS procfs observation (macOS portability)

This observation site reads Linux's process filesystem, which ordinary macOS does not provide. The test therefore fails
for an unavailable measurement substrate rather than checking upload memory behavior. Use a platform-appropriate RSS
observation or explicitly constrain this proof to a supported substrate. The separate hard-coded page-size conversion is
another finding and is not repaired by changing this file read alone.

## Evidence and triage context

- crates/farhelm-supervisor/src/files.rs:1526–1545: the ungated test requires /proc/self/statm and multiplies resident
  pages by 4096.
- crates/farhelm-supervisor/src/files.rs:1530–1531,1557–1573: 64 MiB streamed data is checked against 16 MiB measured
  growth.
- crates/farhelm-supervisor/src/lib.rs:34–49: the crate supports Unix platforms and exports files unconditionally.
- .config/nextest.toml:9–58: no platform exclusion covers this test.
- crates/farhelm-supervisor/src/files.rs:726 gates the module only on test; line 1526 adds no Linux condition.
- crates/farhelm-supervisor/src/files.rs:1537 requires /proc/self/statm; line 1557 invokes it in the child and line 1596
  fails the parent when the child fails.
- crates/farhelm-supervisor/src/lib.rs:34 describes support for Linux and macOS, and line 49 includes files.
- crates/farhelm-testtrace-macros/src/lib.rs:65 adds an ordinary test without a platform restriction.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No matching accepted platform restriction, Planned item, BUGS entry or queue/ledger item found.

Caveats:

- No macOS or large-page Linux execution was performed.
- With 64 KiB pages, a true 64 MiB resident increase is reported as approximately 4 MiB.
- Neither macOS nor large-page Linux execution was performed.
- These are test portability and oracle defects, not evidence that production uploads buffer whole files.
- Verified statically; no macOS execution.
- This finding concerns the test, not production upload streaming.
- Static verification; no macOS execution.
- Separate from the page-size arithmetic defect despite sharing the helper.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_01:p1:F3`,
`gap_supervisor_state_sec_01:p1:F1`.

- `gap_supervisor_state_cor_01:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
- `gap_supervisor_state_sec_01:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.
