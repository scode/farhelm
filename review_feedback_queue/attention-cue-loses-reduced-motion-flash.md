# The attention cue can disappear under reduced motion

Reviewed commit: 806ac6bcbe07b9f9cbb7577647d9ec5a5217f729

## TLDR

On Linux, if the desktop reports reduced motion after Farhelm has enabled animation, an attention cue can show neither a
flight nor a flash.

## Details

The cue chooses its motion branch from Farhelm’s startup setting at `crates/gui/farhelm-gui/src/layers/cue.rs:54–59` and
`:130–143`. The pinned GPUI dependencies can later set the app’s separate reduced-motion flag from the desktop portal.
GPUI then advances the moving animation to its final frame, which the cue draws at zero opacity. SPEC.md, Status
describes a stationary flash in reduced-motion mode. Derive both branches from the same preference. The source
establishes the differing flags if a portal result arrives. Portal delivery in a running desktop remains unverified.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `attention-cues/a8`.
