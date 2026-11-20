# Attention cue tests miss visible activation and paint

Reviewed commit: 806ac6bcbe07b9f9cbb7577647d9ec5a5217f729

## TLDR

The cue tests can pass without showing that an initially offscreen cue is refused, a visible cue plays when the window
becomes active, or reduced motion paints a flash.

## Details

`crates/gui/farhelm-gui/src/layers/cue.rs:397–433` reuses an already-occupied target when checking offscreen admission.
Duplicate rejection can satisfy that assertion. The test invokes activation directly, and the stationary case at
`:347–376` checks state and expiry without measuring the flash. Use an unowned clipped target and a separate visible
target that exercises the real activation observer and paint. These are source-confirmed fixture and witness gaps.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `attention-cues/a4`.
