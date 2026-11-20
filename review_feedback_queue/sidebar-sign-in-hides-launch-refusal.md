# Sign in can silently refuse outside Hosts

Reviewed commit: de61bc055404a995a832cd3d573f54440b6c5954

## TLDR

Sign in from the sidebar or the session card can appear to do nothing while a launch is pending. The reason shows only
inside Hosts.

## Details

The no-question action calls Retry from `crates/gui/farhelm-gui/src/sign_in.rs:550–559`. The launch guard in
`crates/gui/farhelm-gui/src/hosts/remedies.rs:227–236` returns before the command or its ordinary refusal popover and
places its message in Hosts. Show the reason at the button the user acted on or present the guarded action as
unavailable. The source confirms the refusal path and its presentation scope.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `host-attention/l6`.
