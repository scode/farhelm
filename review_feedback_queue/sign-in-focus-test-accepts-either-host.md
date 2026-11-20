# The sign-in focus test accepts either host’s field

Reviewed commit: de61bc055404a995a832cd3d573f54440b6c5954

## TLDR

The explicit sign-in test can pass if the focused popover belongs to the other host in the fixture.

## Details

At the pinned tip `crates/gui/farhelm-gui/src/sign_in/tests.rs:402–426` proves the retry targets the intended host and
that the other question alone cannot open. Once both are present, the final matcher accepts an input ID shared by both
prompts. Assert the focused popover’s host or its distinct question label in the same frame. The existing source
confirms the missing subject check. The fixture is the established scope.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `sign-in-ui-tests/l2`.
