# The Settings spec gives the old sound switch count

Reviewed commit: de61bc055404a995a832cd3d573f54440b6c5954

## TLDR

The Settings specification says the Sounds group has three switches. The detailed sound specification and the desktop
show four.

## Details

At the pinned tip SPEC.md, Settings at lines 2481–2483 retains the old count. The Status and Supported user environments
sections describe the additional sign-in sound and its default. The app draws the four together in
`crates/gui/farhelm-gui/src/settings.rs:1239–1265`. Remove the count or update it to match the detailed contract.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `sign-in-preferences/f1`.
