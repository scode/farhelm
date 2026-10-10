# RSS measurement undercounts on larger-page Linux systems

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Large Linux pages make the memory test accept whole-upload buffering.

## Details

F198 — **definite** — `crates/farhelm-supervisor/src/files.rs:1544` — RSS measurement undercounts on larger-page Linux
systems

The RSS conversion assumes 4 KiB pages. On a 64 KiB-page system, a 64 MiB resident increase is reported as only 4 MiB
and passes the 16 MiB threshold. The test therefore admits the exact whole-upload buffering regression it claims to
reject. Read and validate the actual operating-system page size before converting resident pages to bytes.

## Evidence and triage context

- crates/farhelm-supervisor/src/files.rs:1539 reads resident page count and line 1544 always multiplies by 4096.
- crates/farhelm-supervisor/src/files.rs:1530 sets a 64 MiB upload and line 1531 a 16 MiB allowance.
- crates/farhelm-supervisor/src/files.rs:1557 measures growth through this conversion.
- crates/farhelm-supervisor/src/files.rs:1526–1545: the ungated test requires /proc/self/statm and multiplies resident
  pages by 4096.
- crates/farhelm-supervisor/src/files.rs:1530–1531,1557–1573: 64 MiB streamed data is checked against 16 MiB measured
  growth.
- crates/farhelm-supervisor/src/lib.rs:34–49: the crate supports Unix platforms and exports files unconditionally.
- .config/nextest.toml:9–58: no platform exclusion covers this test.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No requirement limiting this test to 4 KiB-page Linux and no matching disposition found.

Caveats:

- Requires a Linux substrate with pages larger than 4096 bytes.
- The arithmetic is confirmed; no such substrate was exercised.
- Conditional on a larger-page Linux substrate.
- Arithmetic verified; no such substrate executed.
- Do not merge with the separately fixable macOS gate defect.
- No macOS or large-page Linux execution was performed.
- With 64 KiB pages, a true 64 MiB resident increase is reported as approximately 4 MiB.
- Neither macOS nor large-page Linux execution was performed.
- These are test portability and oracle defects, not evidence that production uploads buffer whole files.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_sec_01:p1:F3`,
`gap_supervisor_state_cor_01:p1:F3`.

- `gap_supervisor_state_sec_01:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
- `gap_supervisor_state_cor_01:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
