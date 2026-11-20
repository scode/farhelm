# The below-fold sign-in test checks only the settled frame

Reviewed commit: de61bc055404a995a832cd3d573f54440b6c5954

## TLDR

The test can pass if the popover opens before its sidebar button scrolls into view.

## Details

At the pinned tip `crates/gui/farhelm-gui/src/sign_in/tests.rs:304–322` checks clipped-or-absent before publishing, then
eventual visibility and focus after the harness settles. That permits an earlier popover followed by a later scroll.
Check that the initial button is drawn and clipped, then check the anchor’s visibility when the layer first opens. The
test gap follows from source. Actual placement is unmeasured.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `sign-in-ui-tests/l1`.
