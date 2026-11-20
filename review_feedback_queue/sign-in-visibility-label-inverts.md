# Show and Hide can disagree with the applied visibility

Reviewed commit: de61bc055404a995a832cd3d573f54440b6c5954

## TLDR

The visibility button can do the opposite of its displayed label when the form state changes before the screen updates.

## Details

At the pinned tip `crates/gui/farhelm-gui/src/sign_in_form.rs:169–183` chooses Show or Hide at render, but the callback
calls the live toggle at `:74–80`. Bind the intended masked state directly to the button. This record covers the
source-level label mismatch. Native event timing and any further consequence remain outside the reviewed scope.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `sign-in-ui-tests/l5`, `sign-in-dialog/f2`,
`sign-in-contract/f2`, `attention-cues/a9`.
