# The sign-in handoff test skips the waiting state

Reviewed commit: de61bc055404a995a832cd3d573f54440b6c5954

## TLDR

The startup handoff test can pass without checking that focus moves to a host actually waiting for sign-in.

## Details

At the pinned tip `crates/gui/farhelm-gui/src/sign_in/tests.rs:119–125` and `:190–229` keep both asking hosts Connected
and update only the question collection. A change that skips handoffs for Connecting hosts could leave these assertions
passing. Give the receiving host its real waiting stage before checking focus. This is a source-confirmed test gap.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `sign-in-ui-tests/l4`.
