# Templates can ask to save when only automatic legacy cleanup remains

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A legacy template can ask to save when the only remaining change is automatic cleanup of a retired trust field.

## Details

SPEC.md:1272–1277 limits the leave question to user edits Save would store and expressly excludes removing legacy
workspace trust. Editor::open keeps the legacy fields in the raw baseline and stores the normalized opening draft as
leave_baseline (crates/gui/farhelm-gui/src/templates/editor.rs:263–303). Draft::from_template independently removes
workspace trust and infers the switch (crates/gui/farhelm-gui/src/templates/fields.rs:398–419). The switch control can
restore the stored unset kind without changing any other fields (editor.rs:490–502). That draft differs from the opening
draft because the inferred switch is gone, and differs from storage only because automatic trust cleanup remains. The
whole-draft conjunction added by 7223ac49de6d3d5a67b6aceb1c15b4f75d6646a8 returns true anyway (editor.rs:406–417), and
both departure routes ask (crates/gui/farhelm-gui/src/templates/dialog.rs:422–440, :531–544). The combined legacy
fixture is already present in templates/leave_tests.rs:150–187, but that test puts the inferred switch back before
leaving. The adjacent test leaves with the switch unset but omits legacy trust (:190–209). The endpoint retains the
predicate. Compare the draft with a stored baseline that ignores retired workspace trust for leaving, while preserving
the raw baseline for the footer Save indicator, and cover the two automatic changes together.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `templates/l10`.
