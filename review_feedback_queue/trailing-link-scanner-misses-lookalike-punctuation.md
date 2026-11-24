# The link-text scanner's look-alike table misses common confusables of `.` and `:`

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24

## TLDR

A program can underline text that reads as `paypal.com` but uses a look-alike dot that is not on Farhelm's short list,
such as the Lisu tone mark `ꓸ` or the Syriac full stop `܂`, on a link to another site, and the hover shows no warning.
Likewise `httpsꓽ//paypal.com` with a Lisu colon look-alike is silent. Look-alikes that are on the list (`。`, `․`) do
warn.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer (2026-11-24): a warning bypass where the hover still shows the full real
destination. Related older item: `trailing-osc8-mismatch-warning-dropped-for-unicode-lookalike-hyphens.md` (different
characters, written against the regex this range replaced).

### Reviewer record (`g01-scanner-src/f1`)

Location: `crates/gui/farhelm-grid-model/src/link_claims/fold.rs:52`. Summary: look-alike table omits common confusables
of `.` and `:`, so `paypalꓸcom` or `httpsꓽ//paypal.com` claims nothing

Revision (turn 5): adds spacing-mark look-alikes. U+0903 DEVANAGARI SIGN VISARGA (`ः`, category Mc) is listed in
Unicode's confusables as a look-alike of `:`. It is not in the table, and because it is a mark it is not left alone
either: after a Latin letter it joins that letter's grapheme cluster (UAX #29 GB9a) and `Folded::new` drops it. So
`httpsः//paypal.com` folds to `https//paypal.com`: no `://`, `http` needs a real `:`, `slashes` rejects `//` after the
letter `s`, and `host_may_start` sees `/` before `paypal.com`. No address, no warning. The fix suggested below (derive
the table from confusables.txt) needs to run before mark removal and cover marks, not only punctuation. Related
scanner-side mark handling is in f14. The rest of the record is unchanged.

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24 (moved the table into link_claims and made the scanner depend
on it for recognition); the tip e159ffc79ab147be44e92a92b59571650a4f322d still has the same table (fold.rs:52-61).

TLDR: A program can underline text that reads as `paypal.com` but uses a look-alike dot that is not on Farhelm's short
list, such as the Lisu tone mark `ꓸ` or the Syriac full stop `܂`, on a link to another site, and the hover shows no
warning. Likewise `httpsꓽ//paypal.com` with a Lisu colon look-alike is silent. Look-alikes that are on the list (`。`,
`․`) do warn.

Details: fold.rs:52-61 maps only 7 colon, 5 slash, 4 dot and 2 at look-alikes, some of which NFKC already handles
(U+FF0F, U+FF0E, U+FF1A, U+FF20). Unicode's confusables data lists many more for the structural characters, among them
for '.': U+A4F8 LISU LETTER TONE MYA TI (category Lm), U+0701/U+0702 SYRIAC SUPRALINEAR/SUBLINEAR FULL STOP (Po), U+0660
ARABIC-INDIC DIGIT ZERO and U+06F0 (Nd); for ':': U+A4FD LISU LETTER TONE MYA JEU (Lm), U+0589 ARMENIAN FULL STOP,
U+05C3 HEBREW SOF PASUQ, U+0703/U+0704, U+205A; for '/': U+30CE, U+4E3F, U+27CB, U+1735. Consequences traced through
scan.rs at the tip: (1) `paypalꓸcom`: U+A4F8 is \p{L}, so NAME (scan.rs NAME regex) matches `paypalꓸcom` as one label
with no dot; nothing qualifies, no address, `judge` reports no mismatch. With a Po dot such as U+0702 the token splits
into names `paypal` and `com`, neither of which qualifies; same result. With U+0660 (\p{N}) it is one label again. (2)
`httpsꓽ//paypal.com`: no `://` in the view so `scheme` does not anchor; `http` requires a real `:` after `https`;
`slashes` rejects `//` because `after_letter` is true (U+A4FD is a letter); `host` rejects `paypal.com` because
`host_may_start` sees `/` before it as a barrier in both readings. No address, no warning. SPEC.md (Terminal, OSC 8
paragraph) says 'Look-alike punctuation ... and invisible characters are folded away to recognize an address; an address
that needed that in its scheme or site ... always warns', and its gap list ('The warning is an aid, not a guarantee:
...') does not mention an incomplete look-alike set. Fix: derive the table from Unicode confusables.txt for `.`, `:`,
`/`, `@` (and `\\` if relevant), filtered to characters NFKC does not already map, and add tests for a Lm and a Po dot
look-alike and a Lm colon look-alike; or list the limitation in SPEC.md's gap list. Verify by reading fold.rs:52-61,
scan.rs `NAME`, `slashes`, `after_letter`, `host_may_start`.

Suggested bucket: high. A spoof that the warning exists to catch passes silently with a single substituted character;
not `highest` because the hover still always shows the real target with its host emphasized; if the maintainer treats a
missed spoof warning as security-impacting in itself, `highest`.

Confidence: likely. Source establishes the scanner path for each character (by general category); the unverified premise
is how convincing each glyph is in the terminal font Farhelm uses (U+A4F8 and U+A4FD are listed as confusables of '.'
and ':' in Unicode's confusables.txt; rendering not checked).

proposed_drop: none. No spec section accepts an incomplete look-alike set.

possible_cover: none known.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g01-scanner-src/f1`.
