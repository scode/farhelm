# Narrow-base ZWJ widths retain a concrete-classification source gap

Reviewed commit: fdfcc98cf4062345c4488a388459fcc19d9f595a

## TLDR

A one-cell base followed by ZWJ and a two-cell pictograph may leave tmux and the browser disagreeing about following
cells. The shipped trie classification of a concrete failing sequence remains unestablished.

## Details

`crates/farhelm-ui/assets/vendor/addon-unicode-graphemes.js:1` at zero-based offsets 499, 2115 and 7375 computes
properties, joins and takes the maximum constituent width. Shipped `assets/vendor/xterm.js:1`, offset 163773, advances
by that width increase. Verified tmux 3.7c joins after trailing ZWJ but retains the base width unless a separate
force-wide condition applies (`screen-write.c:2676–2683,2706–2721,2740–2746`; `utf8-combined.c:49–64,104–200`). The
structural disagreement is established; an individual pictograph's classification in the shipped compressed trie remains
the missing source premise.

Suggested bucket: `other`. Confidence: likely conditional interface lead; no concrete rendered failure, occurrence rate
or security/work-loss route is established. Generic simplified ZWJ segmentation alone is not a defect because tmux also
joins those shapes. Existing Unicode tests lack this sequence. Possible cover: the vendored-upgrade policy in
`SPEC_impl.md:1098–1123` may apply but does not expressly settle cross-component parity. Provenance: `g073/f1`. No
runtime or browser reproduction ran.
