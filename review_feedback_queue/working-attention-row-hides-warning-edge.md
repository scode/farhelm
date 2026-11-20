# Live progress can hide an unacknowledged warning edge

Reviewed commit: 806ac6bcbe07b9f9cbb7577647d9ec5a5217f729

## TLDR

Starting work on a host can remove its attention row’s amber warning edge before an earlier setup or update warning is
acknowledged.

## Details

The host’s warning remains latched during a retry in `crates/gui/farhelm-gui/src/hosts/status.rs:284–319`. The attention
projection selects a Working row and clears the edge inputs at
`crates/gui/farhelm-gui/src/session_list/attention.rs:222–252`, and the edge renderer at `:923–959` uses those inputs.
SPEC.md gives progress priority for the row’s content and keeps the amber edge while the warning is unresolved. The host
rail still carries the warning. Keep the progress content and warning edge independent. The source establishes the
projection and paint condition.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `attention-cues/a15`.
