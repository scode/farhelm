# The chime switch test can pass before its draft clears

Reviewed commit: de61bc055404a995a832cd3d573f54440b6c5954

## TLDR

The chime test can pass while the switch still shows its draft choice, even if it never starts following the saved
setting.

## Details

The final case in `crates/gui/farhelm-gui/src/settings/tests.rs:1109–1124` pushes the same value the draft already
displays. `crates/gui/farhelm-gui/src/settings/choices.rs:398–407` lets either the draft or the saved value satisfy the
drawn-position assertion. Push a different saved value after the reply and check that the switch follows it. The test
gap follows from source.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `sign-in-preferences/l5`.
