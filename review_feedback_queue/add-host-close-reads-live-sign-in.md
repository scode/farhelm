# Add Host’s close action can change after it is drawn

Reviewed commit: fc361c4ba6a00ebe69be51fc81fe07ae034a1959

## TLDR

Clicking Cancel SSH sign-in can cancel a later question or close Add Host and discard the destination and alias you
typed.

## Details

At the pinned tip the label at `crates/gui/farhelm-gui/src/hosts/add.rs:1529–1541` uses the drawn question. The dialog
close callback at `:1383` and the live dismissal hook at `:841–858` act on whichever form is current. The direct form
Cancel already binds its own question. Carry that same intent for the header action and keep draft dismissal separate.
SPEC.md, Supported user environments promises that cancelling inline sign-in keeps the draft. The render mismatch is
source-confirmed and its timing was not measured.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `maintenance-ui/l1`, `manual-help/l2`,
`maintenance-contract/f1`, `sign-in-contract/f3`.
