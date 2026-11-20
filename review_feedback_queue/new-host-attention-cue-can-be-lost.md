# A new attention cue can disappear behind an older one

Reviewed commit: 806ac6bcbe07b9f9cbb7577647d9ec5a5217f729

## TLDR

A new reason to sign in can lose its visual cue when the host already has a cue pending or playing.

## Details

Freshness considers a new attempt separately in `crates/gui/farhelm-gui/src/session_list/attention.rs:373–418`. On a
continuously drawn Waiting row, a new attempt can share the old layer target at `:923–934`. The layer rejects that
occupied target in `crates/gui/farhelm-gui/src/layers/cue.rs:120–136`, after freshness has already been recorded. A
fully clipped old cue can keep that slot until it expires. Use one deliberate coalescing or replacement policy across
freshness and layer admission. The same-target case is source-confirmed. Timing and the intended animation frequency on
a continuously visible row remain open.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `attention-cues/a2`.
