# A queued Add-host dialog inherits an elapsed arm timer

Reviewed commit: 579783dd50938967b2c851b8d0b2f855df48c978

## TLDR

The next queued agent Add-host dialog can become interactive without a fresh pause. Whether every new dialog needs that
pause is ambiguous; no harmful carried-over input sequence was established.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/approvals.rs:213-235`. Locations and revisions in the assessment below identify
the reviewed source; later main is outside this review's pinned range.

Synthesis of g299/f3 at reviewed 579783dd50938967b2c851b8d0b2f855df48c978 retains the initial verdict: DISAGREE with
dropping the omission; AMBIGUOUS dialog-mount interpretation and harmful extension mean retain conditionally. The
changed queue-order test (e2e/tests/approval-layout.spec.ts:281-305) checks ordering/expiry but never asserts disabled
answers or advances a controlled clock at the next dialog mount. Existing tests of normal receipt and two windows do not
establish old-form fallback safety (:201-209,380-389). a018's automatic-opening pause refutation names a different
trigger: initial listing/selection changes do arm; g299/f3 concerns promotion of a later queued opening after that timer
elapsed while its listing/expanded IDs stay unchanged. g300/f3=g301/f1 is a separate manual-setup submission clearing a
parked opening; it prevents that particular next dialog from appearing, but does not refute advancement after an
accepted agent add closes. a018 alone owns the old-callback retirement refutation; no general stale-native-event safety
is inferred. Suggested OTHER for the established missing fresh-presentation guard; no harmful input sequence, highest
consequence, or material UX impact established. possible_cover remains the explicit list/expanded-trigger wording versus
SPEC_impl.md:5690 dialog coverage, not exact accepted exclusion. proposed_drop=false. Test code inspected only, never
executed.

Provenance: `a072/f4`, `g299/f3`. These are reviewer-local identities, not additional findings. Proposed drop: none for
the retained remainder.
