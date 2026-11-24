# A spacing mark drawn in the background colour counts as seen and silences the warning without a hard-to-see note

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e

## TLDR

On one row, a program can underline `/  paypal.com/login` where the gap is a spacing mark drawn in the background
colour, and point the link elsewhere. The user sees a slash, a blank and a familiar address; Farhelm reads the slash
glued to the address, which makes it a file path rather than an address, and shows no warning, not even that part of the
text is hard to see.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer (2026-11-24); g03 suggested `highest`. g07 confirmed the premise for
marks that draw ink (spacing combining marks, U+0600-0604, U+06DD, U+110BD, U+110CD) but not for U+2800 or U+115F, which
draw nothing in any colour. Root cause in colors.rs (g07 c6), consequence in the hover (g03 f10), and the scanner side
is `trailing-link-scanner-spacing-mark-after-slash-hides-host.md`.

### Reviewer record (`g03-disclosure-src/f10`)

Location: `crates/gui/farhelm-grid-model/src/link_disclosure.rs:390`. Summary: Consequence of g07 c6 in the hover: a
background-coloured spacing mark glues a visibly separate address to `/` and silences it

Reviewed commit: ba0da50536236ca934f3956d67a0c35e4b57586e (the hover's label reader and hidden evidence); the tip
e159ffc79ab147be44e92a92b59571650a4f322d still has it. Root cause is g07 c6's (colours: a run made only of
cell-occupying marks drawn in the background colour is not unseen); recorded here only for the concrete spoof route
through the hover, for dedup with g07 c6.

TLDR: On one row, a program can underline `/  paypal.com/login` where the gap is a spacing mark drawn in the background
colour, and point the link elsewhere. The user sees a slash, a blank and a familiar address; Farhelm reads the slash
glued to the address, which makes it a file path rather than an address, and shows no warning, not even that part of the
text is hard to see.

Details: `label_row` (crates/gui/farhelm-grid-model/src/link_disclosure.rs:358-423) takes a run into the label when its
visible identity is the hyperlink's (`links.rs` `visible_link`, 170-179, which drops only `colors::unseen` runs) and
counts hidden evidence only for runs where `hides_ink` holds (links.rs:332-334, requiring `colors::draws_something`).
Per g07 c6, a run consisting of a spacing mark such as U+0903 (width 1, its own cell; unicode-width 0.2.3 rule 4) in the
background colour is neither unseen nor ink-hiding, so it stays in the stretch text and adds no Hidden. One-row label
`/ःpaypal.com/login` to `https://evil.test/`: `/` and `ः` form one grapheme cluster (GB9a) that folds to `/`
(link_claims/fold.rs:100-111), so `Scanner::host_may_start` (scan.rs:587-615) refuses the host after `/`; no address, no
reason, `verdict` returns `None`. With the mark drawn visibly the text reads glued (`/ःpaypal.com/login`), which the
scanner's path rule reasonably treats as a path; the deception exists only because the mark is invisible. Fix belongs
with g07 c6: treat a cell-occupying mark (or any run that occupies a cell and contrasts below the threshold) as unseen
or as hidden evidence, so it either breaks the label (and the address on its own then warns) or adds Hidden.

Suggested bucket: highest, as a silenced spoofing warning through a program-controlled route; same bucket as g07 c6 if
that is judged otherwise.

Confidence: confirmed in my code given g07 c6's premise about `colors::unseen`/`draws_something` for spacing marks
(their scope; I read only the call sites).

proposed_drop: none. SPEC.md 3628-3630 defines hidden text as having "a character other than whitespace, a format
character or a combining mark", which literally excludes a run of only a spacing mark; that is the spec text g07 c6
bears on, and it does not mention the spoofing consequence.

possible_cover: g07 c6 (root cause).

### Also reported by `g07-colours-faint/c6`

Location: `crates/gui/farhelm-grid-model/src/colors.rs:247`. Summary: The hidden-text test exempts every combining mark
and format character, so cell-occupying marks that draw ink count as seen when drawn in the background colour

Revision of c6, now a finding. g03 f10 showed a consequence in program hyperlink text that my turn-2 refutation, which
covered printed URLs only, did not address. The printed-URL part of that refutation still stands (see the end). This
record covers the classification in colors.rs, which is in g07's scope; g03 owns the disclosure-side record.

Reviewed commit: 997f2e85c10fbeb55ae0056317a5ddc15fc9b0b6 introduced DRAWS_NOTHING and the exemption in colors::unseen.
ba0da50536236ca934f3956d67a0c35e4b57586e reused it for hyperlink labels (visible_link) and the warning evidence
(draws_something, hides_ink). The tip e159ffc79ab147be44e92a92b59571650a4f322d still has it.

TLDR: Farhelm decides that text drawn in its background colour is hidden, unless it consists only of characters it
assumes draw nothing. That assumption covers every combining mark and every format character, but some of them are
ordinary visible symbols that take up a cell of their own. A program can draw one of those in the background colour: the
user sees a blank, while Farhelm counts it as readable. In a program's hyperlink that keeps it in the link text and
stops the hover from saying "Part of the link text is hard to see" (g03 f10 shows how that hides a mismatching address
from the warning).

Details: What g03 relies on, checked exactly. DRAWS_NOTHING (crates/gui/farhelm-grid-model/src/colors.rs:247-251)
matches a run made only of JS whitespace, U+115F, U+1160, U+3164, U+FFA0, U+2800, \p{Cf} and \p{M}. unseen
(colors.rs:273-281) never calls such a run hidden by colour. visible_link (links.rs:178) keeps it in the hyperlink's
label, ranges and underline, and hides_ink (links.rs, draws_something) does not count it as evidence for the 'hard to
see' line. Which of these can be background-coloured on their own: the emulator attaches every width-0 character to the
previous cell without changing that cell's style (vendor/alacritty_terminal/src/term/mod.rs:1527-1548). Farhelm's
patches drop the format characters listed in emoji_ignored_format (term/mod.rs:1442-1450) and attach V/T jamo
(term/mod.rs:1248-1259). So only characters with unicode-width (0.2.3) width >= 1 get a cell with the current colours.
Of the exempt set, those are: (a) Characters that draw ink: spacing combining marks (Mc) that are not Grapheme_Extend,
e.g. U+0903, U+093E, U+0940, U+0BBF, U+0D02; and the prepended concatenation marks U+0600-0604, U+06DD, U+110BD and
U+110CD, which are Cf but have glyphs. In the background colour these draw nothing visible, yet they count as seen. This
is the defect: the colour test is what should catch them, and the exemption stops it. (b) Characters that draw no ink in
any colour: U+2800, U+115F, and the JS whitespace characters other than U+1680. Their colour is irrelevant, because they
look blank in the default colour too. g03's U+2800 variant is therefore an invisible-character question, not this colour
test. U+1680 (Ogham space mark) does draw a dash, but it is whitespace to both the URL regex and the address scanner, so
counting it as a blank is what the text means. (c) U+FFF9-FFFB (interlinear annotation, Cf, width 1): fonts generally
draw nothing, so this is like (b). I did not verify every font's behaviour. So g03's classification holds exactly for
the spacing-mark example (group a) and not for U+2800 (group b). Spec: SPEC.md, printed-URL paragraph (~line 3629):
"Text is hidden when it has a character other than whitespace, a format character or a combining mark". The code
implements this faithfully. The spec's premise that format characters and combining marks draw nothing is false for
group (a). SPEC_impl.md (~line 2503) repeats it: "calls a run hidden when it has something to draw". Fix: base the
exemption on what draws, not on the category. Drop \p{Mc} and the prepended concatenation marks (U+0600-0605, U+06DD,
U+070F, U+0890-0891, U+08E2, U+110BD, U+110CD) from DRAWS_NOTHING, keeping Mn/Me (always width 0, so they share their
base's colour) and the remaining Cf. Alternatively exempt only known blanks (whitespace, the Hangul fillers, U+2800) and
zero-width characters, since zero-width characters cannot be coloured apart from their base anyway. Add a colors.rs test
case with U+0903 in the background colour, and update the SPEC.md / SPEC_impl.md sentence to match. Printed URLs (the
turn-2 analysis, still valid): every group (a) character is non-ASCII, and host_prefixed (links.rs:143-167) refuses a
printed URL whose host or userinfo is not ASCII. So there such a mark can only add percent-encoded bytes to a path,
query or fragment, and it shows as a one-cell gap.

Suggested bucket: other for the colors.rs defect on its own. For printed URLs it adds only invisible path bytes; for
hyperlinks it weakens the warning, which SPEC.md calls "an aid, not a guarantee" while "the destination display stays
authoritative". If triage merges it with g03 f10, it takes that record's bucket, since this is the root cause of that
consequence.

Confidence: confirmed that group (a) characters get their own cell with the current colours (emulator source and
unicode-width rules) and are exempt (DRAWS_NOTHING). That they draw visible ink in the default colour follows from their
being spacing marks and concatenation marks with standard glyphs. Which font draws them is not checked, and it does not
change the point: in the background colour they are invisible either way.

proposed_drop: none. The SPEC.md wording quoted above accepts the exemption, but its premise is false for group (a), as
with f1.

possible_cover: g03 f10 (disclosure-side consequence of the same exemption).

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g03-disclosure-src/f10`, `g07-colours-faint/c6`.
