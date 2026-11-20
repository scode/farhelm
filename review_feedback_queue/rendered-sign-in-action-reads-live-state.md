# Answer can switch to a different action after redraw is queued

Reviewed commit: de61bc055404a995a832cd3d573f54440b6c5954

## TLDR

A visible Answer button can open a later question or start a new retry if the displayed question ends before the click
is handled.

## Details

At the pinned tip, `crates/gui/farhelm-gui/src/sign_in.rs:511–558` draws the label from state at render and resolves the
current question or host phase again at activation. Binding a question ID later protects only the action selected by the
callback. Capture the displayed Answer or Retry intent at render and decline an obsolete intent. The source confirms the
binding defect. The established effect is a change in which question opens or whether the retry starts. The timing
remains unmeasured. Several reviewers suggested high because the visible action changes during sign-in. The recorded
consequence is limited to opening a different question or starting a retry.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `sign-in-dialog/f1`, `sign-in-ui-tests/f1`,
`host-attention/f1`, `sign-in-broker/l5`, `sign-in-contract/f1`, `maintenance-contract/f2`, `maintenance-broker/l6`,
`attention-cues/a3`.
