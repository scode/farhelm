# A teardown failure hides the drift check’s original result

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A teardown failure can hide the original launch or Restart failure in the installed-agent check.

## Details

crates/farhelm/tests/e2e/real_agent_capture/drift.rs:650–673 keeps the initial check result in memory, including screen
and hook diagnostics from :470–475 and :553–575. At :674–678, cleanup? exits before returning that result, and :683–700
writes and prints only the resulting error. The runner retains that detail in scripts/run-harness-drift.py:154–160, so
the earlier diagnostic from this route never reaches the evidence recorder. Delete explicitly can fail when the
supervisor cannot confirm teardown or the transport goes away (crates/farhelm-helm/src/client.rs:3802–3813 and
:3841–3863). Keep a failed overall verdict for cleanup errors while preserving both results when the check also failed
or was blocked. No later change touches drift.rs in the pinned range.

Confidence: confirmed by inspection for simultaneous check and cleanup errors. Their incidence was not measured.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `driftRunner/l10`.
