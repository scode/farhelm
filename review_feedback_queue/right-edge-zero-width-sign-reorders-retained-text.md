# Right edge zero width sign reorders retained text

Reviewed commit: a40a016d02a34881464725023083aa6684b33092

## TLDR

With wrapping disabled, zero-width terminal output at the right edge can attach to the preceding cell and change
retained text order.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: highest.

Recording judgment: The recorder retains highest for corruption of retained terminal text, conservatively preserving the
possible user-work consequence. The owner's suggested other placement is preserved below; no lowering or exclusion is
being acted on. The stored/selected order is source-confirmed, while no irreversible lost file, harmful command or
observed incident is claimed.

Source location: `vendor/alacritty_terminal/src/term/mod.rs:1535 at a40a016d02a34881464725023083aa6684b33092`.

Source assessment (read with the later corrections below) (review record g078/l2, event 12):

Additional source confirmation of l2; the preceding finding event contains the complete two-column ESC[?7l a U+2764
U+1161 U+FE0E reproduction and target mismatch. Reviewed full hash a40a016d02a34881464725023083aa6684b33092. TLDR: a
newly retained zero-width sign can be attached to the wrong character at a nonwrapping row edge, reordering selected
text and failing to block a following selector. Directly read pinned UI source rather than relying solely on neighbor
synthesis: grid-buffer.js:69-91 emits a whole non-ASCII run at its start cell, and :126-148 trims ASCII spaces only and
normalizes NBSP, so U+1161 and FE0E survive in the wrong stored order. Plain capture also emits cells then suffixes at
terminal-emulator/src/screen.rs:86-89. The exact cached dependency requested by Cargo.lock:6713-6716 is unicode-width
0.2.3; its src/lib.rs:100-105 specifies width zero for Vowel_Jamo and Trailing_Jamo. Its char-width implementation at
:226-227 forwards to lookup::single_char_width, whose normal-width table path is lookup.rs:49-59. These checks support
the zero-width fallback premise without running code. Pinned git history from reviewed hash to
8a8c9579e04299410652f0e4681975a7198dfc7b is empty for mod.rs, Cargo.lock, grid-buffer.js and screen.rs. Confidence
remains confirmed source inspection, not an unverified-after-investigation finding and not a runtime observation.
Suggested bucket other; no established irreversible-work-loss or security route. proposed_drop false. possible_cover not
searched; known g030:f11 and g031:g031-l1 are distinct. Caveats unchanged: the generic zero-width target logic predates
the assigned repair, but a40a016 newly sends unattached V/T into it; fonts and clipboard availability do not affect the
stored/selected-text result. No remaining missing premise was identified.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g078/l2`; supporting records
`g031/g031-f6`. The retained private log preserves earlier wording and its corrections. This record does not claim
runtime reproduction.
