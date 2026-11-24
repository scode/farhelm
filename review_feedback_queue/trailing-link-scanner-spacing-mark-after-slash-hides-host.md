# A spacing mark right after `/` hides the host that follows, even on one row

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24

## TLDR

Link text `/ःpaypal.com/login`, where `ः` is a Devanagari mark that takes its own cell and looks like a colon (or is
drawn in the background colour so the cell looks empty), on a link to another site shows no warning. The plain-ASCII
equivalents `/:paypal.com/login` and `/ paypal.com/login` both warn.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer (2026-11-24): a warning bypass where the hover still shows the full real
destination. Interacts with `trailing-link-disclosure-letter-test-counts-spacing-marks.md` and
`trailing-link-text-invisible-spacing-mark-silences-warning.md`.

### Reviewer record (`g01-scanner-src/f14`)

Location:
`crates/gui/farhelm-grid-model/src/link_claims/fold.rs:100-120; crates/gui/farhelm-grid-model/src/link_claims/scan.rs:587`.
Summary: a spacing mark after `/` or `\` merges into that character's cluster and is folded away, so a host drawn a cell
later reads as glued to a path and claims nothing

Scanner-side cause of g03's f9 (rows joined across `/` + spacing mark) and f10 (one row, `/`, a background-coloured
spacing mark, then the address), from the scanner's side.

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has it
(fold.rs `Folded::new` and `lookalike`, scan.rs:587 `host_may_start`).

TLDR: Link text `/ःpaypal.com/login`, where `ः` is a Devanagari mark that takes its own cell and looks like a colon (or
is drawn in the background colour so the cell looks empty), on a link to another site shows no warning. The plain-ASCII
equivalents `/:paypal.com/login` and `/ paypal.com/login` both warn.

Details: How the scanner treats marks and format characters, from source. (a) Grapheme clustering (unicode-segmentation,
extended clusters) attaches Extend (Mn, Me, ZWJ/ZWNJ) and SpacingMark (most Mc, e.g. U+0903) to the preceding character
(GB9/GB9a), so `/ः` is one cluster with `base` `/`. A mark at the token start, or after a separator, is a cluster of its
own whose `base` is the mark. Prepend characters (U+0600-0605, U+06DD, ...) attach to the following character instead
(GB9b). (b) `Folded::new` keeps marks only when the cluster's `base` is a letter of a script other than Latin, Greek or
Cyrillic (`OTHER_SCRIPT_LETTER`); otherwise every \p{M} is dropped, and every \p{Cf} is dropped regardless. A dropped
mark sets `changed`, which matters only inside the scheme, authority or authority-ending character of an address that is
found (it then reads as Disguised). (c) Neither `letter()` (\p{L}) nor the `barrier` in `host_may_start` treats a mark
as a letter; marks are simply invisible to recognition. So in `/ःpaypal.com/login` the original reading's previous
cluster has `base` `/` and the folded view reads `/paypal.com/login`: both readings see a path barrier and `paypal.com`
never anchors. The same happens after `\` and `@` (the latter also f5). After a letter, digit or `.`, the folded view
glues the neighbours into one name that still anchors and comes out Disguised, so those warn. A lone mark at the token
start or after a separator folds to nothing and the address after it is found normally. Fix point: in `host_may_start`,
let a host start when the cluster before it (original reading) holds a character that occupies its own cell and is
dropped by folding (a spacing mark, or more simply any \p{Mc}); equivalently, fold a SpacingMark that follows a
non-letter base to a space for boundary purposes. Separately, U+0903 is listed in Unicode's confusables as a look-alike
of `:`, which is also why `httpsः//paypal.com` is silent (see the f1 revision).

This is also why g03's proposed fix for their f9 (judge 'a letter before the address' by Unicode letter category, as the
scanner does) is not enough alone: with that change the first reading separates the seam and mismatches, but the joined
reading `/ःpaypal.com/login` still finds no address, and link_disclosure.rs cancels on an empty joined reading (my f9).
Both changes, or this scanner-side change, are needed.

Suggested bucket: high. A sender-controlled two-character prefix silences the warning for a familiar address; not
`highest` only because the hover always shows the real destination in full with its host emphasized; if the maintainer
treats a missed spoof warning as security-impacting in itself, `highest`.

Confidence: confirmed for the scanner path (source and UAX #29 rules GB9/GB9a/GB9b as implemented by
unicode-segmentation's extended clusters); that U+0903 occupies its own cell in Farhelm's grid is taken from g07 c6 and
g03 f10, not verified here.

proposed_drop: none. SPEC.md's gap list does not include it; '`/tmp/example.test/x` (a path)' is the intended barrier,
but here the drawn text shows a separate cell between `/` and the host.

possible_cover: g03 f9 and f10 (same symptom, disclosure-side analysis); g07 c6; my f9 for the joined-reading
cancellation.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g01-scanner-src/f14`.
