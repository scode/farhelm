# Save and Close can hide which hosts missed a credential replacement

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

Save and Close can hide which hosts failed to receive a saved credential replacement.

## Details

credentials/editor.rs:310-323 treats committed replacement responses as Done, hands the host failures to
credentials.rs:389-417, and signals a successful settings receipt. The overview's local lines are built at
credentials.rs:558-575. The receipt defers to the same advance_batch that closes Settings when the last draft is clean
(editing.rs:84-88,438-440,453-465), so closing discards those lines without showing them. The durable host warning
remains visible elsewhere (SPEC.md:491-496 and SPEC_impl.md:5966-5974). This finding concerns the required replacement
outcome in SPEC.md:2482-2483. Mitigation: preserve the committed outcome and open the Credentials overview on delivery
warnings, or surface the same host-by-host outcome outside the closing dialog.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `footerEditors/l8`.
