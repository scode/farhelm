# Attention cue expiry can get ahead of its first frame

Reviewed commit: b6e33332cd8beab675da31b04509fbbee70be8bb

## TLDR

If a visible cue’s first frame is delayed long enough, its expiry timer can remove it before the landing flash.

## Details

Unverified after investigation. The retirement timer starts on admission at
`crates/gui/farhelm-gui/src/layers/cue.rs:166–183`. GPUI starts the moving animation on its first layout at
`vendor/gpui-pre/src/elements/animation.rs:394–426`. The landing flash starts after 68% of the animation at
`layers/cue.rs:192–199`. The expiry and frame paths have independent start times. The checked platform scheduler has no
matching delivery guarantee. The missing premise is a visible window where first layout is sufficiently delayed and
retirement runs before any landing frame. A correlated first-layout, paint and expiry trace would settle it. Use a
shared start for paint and retirement if the condition is reachable.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `attention-cues/a10`.
