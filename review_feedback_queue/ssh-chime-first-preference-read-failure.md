# An initial preference failure can play a disabled chime

Reviewed commit: de61bc055404a995a832cd3d573f54440b6c5954

## TLDR

Farhelm may play the sign-in chime despite a saved opt-out if its first preference read fails while a question is
pending.

## Details

The driver at `crates/gui/farhelm-gui/src/sounds/owner.rs:148–163` passes the missing singleton to the default-on sound
rule. A failed initial collection read is represented separately from a successful read of unset preferences by
`crates/farhelm-helm/src/ui_channel.rs:760–797`. A later failed refresh can retain an earlier value, so the current
issue is limited to a fresh UI session before it has seen the saved preference. Keep the question eligible until a
preference is known. The branching is source-confirmed. The simultaneous read failure and audible chime are unverified
after investigation.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `sign-in-preferences/l1`.
