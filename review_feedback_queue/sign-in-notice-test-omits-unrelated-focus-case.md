# The sign-in notice test misses an unrelated follow-up

Reviewed commit: de61bc055404a995a832cd3d573f54440b6c5954

## TLDR

The tests can pass if a question from a different sign-in takes focus when an open notice ends.

## Details

The notice-transition fixture at `crates/gui/farhelm-gui/src/sign_in/tests.rs:186–196` supplies a follow-up with the
same attempt ID. The later negative check at `:205–220` covers the separate post-submit path. SPEC.md, Who may move
focus, limits the immediate handoff to the same attempt. Add an unrelated question at the notice-removal boundary and
observe that the attention row appears while the popover stays closed. The source establishes the missing test case.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `sign-in-ui-tests/l6`.
