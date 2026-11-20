# Attention cue admission accepts an empty intersection

Reviewed commit: b6e33332cd8beab675da31b04509fbbee70be8bb

## TLDR

A moving attention cue could travel toward an invisible row if its scroll area collapses inside the row.

## Details

Unverified after investigation. The tracker in `crates/gui/farhelm-gui/src/layers.rs:648–661` can return an empty but
present intersection, and the cue at `layers/cue.rs:120–123` and `:208–225` checks only whether one exists. The geometry
predicates allow a zero-width or zero-height rectangle strictly inside the row. Scene insertion discards the zero-area
stationary quad but the moving dots have their own positive bounds. The inspected scroller and normal platform size
rules did not establish an ordinary layout with the collapsed edge inside the anchor. A complete layout derivation or
correlated clip/row bounds would settle it. Reject empty intersections at the tracker boundary.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `attention-cues/a11`.
