# Rust sandbox packaging ignores Cargo target-directory overrides

Reviewed commit: f17458c5ed6159cac9f621cc2350360d00fa396c

## TLDR

A checkout built only into a shared or external Cargo target directory cannot package its Rust sandbox tests: the runner
still requires its fixture binary under the default target path.

## Details

Suggested bucket: **other**. Recorder confidence: **verified**. The original reviewer's confidence, qualifications and
severity history are retained below; source-confirmed conditional behavior is not a runtime observation.

Shared Cargo targets are explicitly supported by repository machine guidance. No contract restricts this runner to
default-target-only builds, and the old relative-target desktop documentation issue is a different consumer. This is a
repeatable configuration failure, not a transient safe-retry glitch.

## Recorded source assessment

Reviewed commit f17458c5ed6159cac9f621cc2350360d00fa396c. TLDR: developers building with a shared/external Cargo target
directory cannot stage their Rust sandbox tests using the documented build procedure. Checked rust-tests.ts:23-25,47-55;
.agents/tensorlake.md:70-88; crates/farhelm/tests/e2e/harness.rs:505-530; repository Cargo-target guidance. cargo build
and cargo nextest archive honor CARGO_TARGET_DIR, but pack always tars REPO/target/debug/farhelm-fixtures at :55. For a
fresh checkout built only with an external target, that path does not exist, so tar rejects before create/stage :62 and
no sandbox tests can run. A stale file at the default path is also the wrong source of fixture identity, but no separate
wrong-binary execution claim is made because nextest compiled-path remapping was not established from available source.
The e2e harness explicitly requires the fixture beside its compiled CARGO_BIN_EXE_farhelm, further explaining why
choosing the default target copy is not generally correct. Suggested bucket other, maintainer tooling. Confidence
confirmed for pre-stage missing-file failure; no runtime or provider checks needed. proposed_drop false. possible_cover
none checked; the template docs describe the default target path but do not explicitly reject or override
CARGO_TARGET_DIR, while root machine guidance supports shared targets. Fix: derive the built target directory from Cargo
metadata/build inputs and stage the matching fixture at the path the archive tests require, or explicitly
validate/refuse unsupported layouts before packaging with an actionable diagnostic.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g045/l6`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
