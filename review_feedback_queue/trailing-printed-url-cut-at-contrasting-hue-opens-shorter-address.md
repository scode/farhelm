# A printed URL ends at readable text in a contrasting hue, so a plain click opens a shorter address than the one shown

Reviewed commit: 997f2e85c10fbeb55ae0056317a5ddc15fc9b0b6

## TLDR

A program can print an address such as https://examplebank.com/login where the last letters ("m/login") are drawn in a
color about as bright as their background but a clearly different hue, for example red on mid-gray. People with ordinary
color vision can read that text, but Farhelm counts it as invisible and ends the link before it. The user reads
examplebank.com, but a plain click, which shows no destination first, opens examplebank.co. Before this change the whole
printed address linked, so what was shown was what opened.

## Details

**highest.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `highest` per the maintainer (2026-11-24). The code route is confirmed; whether people read text at
that contrast but a different hue is a perceptual premise nobody verified. The fault is in SPEC.md's 1.35:1 rule rather
than in the code that implements it.

### Reviewer record (`g07-colours-faint/f1`)

Location: `crates/gui/farhelm-grid-model/src/colors.rs:273`. Summary: Readable text in a contrasting hue counts as
hidden and cuts a printed URL mid-host, so a click opens a shorter address than the one shown

Revision of f1. It adds where the defect lies (the spec's rule, which the code implements faithfully), the
hyperlink-side effect of ba0da50536236ca934f3956d67a0c35e4b57586e, and the check that this was not possible before. The
premise stays named.

Reviewed commit: 997f2e85c10fbeb55ae0056317a5ddc15fc9b0b6 introduced colors::unseen with the luminance-only test, the
SPEC.md rule and printed-URL blanking. The tip e159ffc79ab147be44e92a92b59571650a4f322d still has all three.

TLDR: A program can print an address such as https://examplebank.com/login where the last letters ("m/login") are drawn
in a color about as bright as their background but a clearly different hue, for example red on mid-gray. People with
ordinary color vision can read that text, but Farhelm counts it as invisible and ends the link before it. The user reads
examplebank.com, but a plain click, which shows no destination first, opens examplebank.co. Before this change the whole
printed address linked, so what was shown was what opened.

Details: colors::unseen (crates/gui/farhelm-grid-model/src/colors.rs:273-281) calls a run hidden when
contrast(glyph(ink), ink.bg) < HIDDEN_CONTRAST (1.35, colors.rs:196). contrast (colors.rs:202) is the WCAG 2 ratio of
relative luminances, so two colors of equal luminance score 1.0 whatever their hue. Examples: #ff0000 on #808080 (both
luminance about 0.21) scores about 1.0; #ff0000 on #009400 scores 1.0; on the real theme's default background #282c34,
dark red #860000 scores about 1.34. Printed URL detection blanks the whole run: walk_cells
(crates/gui/farhelm-grid-model/src/selection.rs:268) yields spaces for it, and row_links
(crates/gui/farhelm-grid-model/src/links.rs:430-489) matches URLS on the blanked text. The match ends at the last seen
cell and still links (doc at links.rs:338-343: "The URL simply ends before such a cell ... its visible part still
links"). host_prefixed only checks that the host is spelled canonically, so a match cut inside the host links as long as
the prefix parses. The cut can fall inside the host, giving a different registrable domain than the text the user reads
(examplebank.co vs examplebank.com), or inside the path (github.com/acme/tools vs github.com/acme/tools-official). Plain
printed URLs have no destination display (SPEC.md: "Plain URLs printed as text need no such display, because what is
shown is what opens"); only the hover underline stopping early hints at it. Why this is new: before 997f2e85c, the only
way to end a printed URL without a visible delimiter was conceal, which draws a blank gap. Background-colored text also
draws as a gap. A printed URL overlapping a program hyperlink is dropped whole, not truncated (links.rs:463-466 clears
ranges). Readable chromatic text is the first route where the link ends with no visible gap. Hyperlink side
(ba0da50536236ca934f3956d67a0c35e4b57586e): visible_link (links.rs:178) applies the same misjudgement to program
hyperlinks. Readable chromatic label text is left out of the label, ranges and underline, and hides_ink makes the hover
say "Part of the link text is hard to see". The destination display stays authoritative, so there it causes false
warnings and a shortened underline, not a spoof. Is the defect in the code or the spec: in the spec's rule. SPEC.md
(printed-URL paragraph, ~line 3627) defines it operationally: "Hidden text, too close to its background color to read,
also breaks a printed URL ... a URL ends where the readable text does. Text is hidden when it has a character other than
whitespace, a format character or a combining mark, and its color contrasts with its background at less than 1.35:1,
judged as Farhelm draws it". SPEC_impl.md (terminal emulator section, ~line 2503) names the measure: "its WCAG contrast
with its background is below 1.35:1". The code implements exactly that, dim blend included, so it is faithful. The
rule's two premises fail for chromatic pairs: that a WCAG ratio below 1.35 means "too close ... to read", and that
ending the URL there makes it end "where the readable text does". Neither spec acknowledges hue or that the cut can land
inside a host. Verify by source: trace a row with runs ["https://examplebank.co" default style] ["m/login" fg
Rgb(255,0,0) bg Rgb(128,128,128)] through unseen, walk_cells and row_links; link_at on col 8 returns uri
https://examplebank.co. A test shaped like a_printed_link_ends_before_hidden_text in
crates/gui/farhelm-grid-model/tests/links.rs would pin it. Fix (spec and code together): (1) fail closed for printed
URLs: when a URLS match is cut by a hidden-by-color run whose source text was not whitespace, do not link it, instead of
linking the prefix. Conceal can keep ending the URL, since it draws a gap. (2) Or add a color-difference test (OKLab or
CIEDE2000 distance) so only text close to its background in hue as well as luminance counts as hidden. Option 1 keeps
the threshold from ever shortening what opens relative to what is read. Either way SPEC.md's "a URL ends where the
readable text does" sentence changes with it.

Suggested bucket: highest. Terminal output can make a plain click open a different site than the address the user reads,
a destination spoof with no disclosure. The only unestablished step is the perceptual premise below; until triage
accepts it, high is the established placement, and accepting it moves the finding to highest.

Confidence: likely. Source establishes that equal-luminance, different-hue text is classed hidden and truncates the
printed URL, including inside the host. Unverified premise: that such text (e.g. red on mid-gray, or dark red #860000 on
the default #282c34) is legible enough for the user to read it as part of the address. The basis is that WCAG 2's
luminance-only ratio is widely documented to ignore hue contrast. I did not check it by observation.

proposed_drop: none. The spec states the rule the code follows, but the defect is that rule's premise; dropping on the
spec's text would keep the hole. If triage disagrees, the section to cite is the SPEC.md printed-URL paragraph quoted
above.

possible_cover: none known.

### Also reported by `g05-gui-disclosure/f8`

Location: `crates/gui/farhelm-gui/src/terminal/attached.rs:2900`. Summary: GUI side of g07: a printed URL cut short by
'hidden' text opens on click with no display

Revision: only the Reviewed commit line changes, to the full hash; everything else as in the previous event for f8.

Reviewed commit: 997f2e85c10fbeb55ae0056317a5ddc15fc9b0b6 ("fix: end printed URLs before text too faint to read", #4763)
introduced the cut that g07 owns; the GUI gating examined here predates the range and is unchanged at the tip
e159ffc79ab147be44e92a92b59571650a4f322d. Recorded as GUI-side evidence for g07's finding, for deduplication.

TLDR: When colored text that is actually readable is judged 'too faint to read', a printed address ends early (the user
reads examplebank.com/login, the link is examplebank.co). Clicking it opens the shortened address straight away: nothing
in the app shows where a printed address goes before opening it, so the user has no chance to notice.

Details: AttachedTerminal::disclosed_link (crates/gui/farhelm-gui/src/terminal/attached.rs:2892-2901) gives a display
only to a file path, a program hyperlink (LinkKind::Osc), or a link with a Mac tunnel hint; a printed address
(LinkKind::Text) without a hint gets none. click_visibility (attached.rs:3273-3285) returns Ok(()) at once for
LinkKind::Text without a tunnel hint, and the release opens link_web_target(&press.link) through cx.open_url
(attached.rs:2821-2850) with no prompt. This follows SPEC.md, terminal links paragraph: 'Links in terminal output open
their http(s) target on click, with no confirmation or prompt of any kind' and 'Plain URLs printed as text need no such
display, because what is shown is what opens'; g07's premise breaks the 'what is shown is what opens' assumption. The
only GUI-side cue is the hover underline (TerminalView::underlined_rows, crates/gui/farhelm-gui/src/terminal.rs:727-743;
paint_link_underline, terminal.rs:1209-1222): a 1-pixel line in the default foreground under exactly the link's cells,
so the readable-but-'hidden' tail is not underlined, plus the pointing hand. That is a faint hint, not a gate. GUI-side
fix options, if g07's classification is not narrowed instead: give a printed address cut next to hidden-classified ink a
display (and so the visibility gate), or show its destination on hover.

Suggested bucket: highest, contingent on g07's premise: a program's output can make a plain click open a different site
than the text the user reads (a spoofing route).

Confidence: likely. The GUI behavior is confirmed from source; the premise that readable text is classified hidden and
cuts a printed URL inside its host is g07's and not verified here.

proposed_drop: none.

possible_cover: g07's finding on hidden-text classification cutting printed URLs; this record adds only the GUI-side
confirmation.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g07-colours-faint/f1`, `g05-gui-disclosure/f8`.
