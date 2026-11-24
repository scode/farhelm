# A real IDN ending that looks like an ASCII one (`.ευ` for `.eu`) is exempt from the look-alike check

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24

## TLDR

Link text `paypal.ευ` (Greek epsilon and upsilon), which reads as `paypal.eu`, on a link to another site shows no
warning. The same text with a Cyrillic `о` in a `.com` ending does warn.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer's rule for warning bypasses (2026-11-24); the reviewer suggested `other`.

### Reviewer record (`g01-scanner-src/f10`)

Location: `crates/gui/farhelm-grid-model/src/link_claims/scan.rs:295`. Summary: a real IDN ending that looks like an
ASCII bare ending (Greek `.ευ` for `.eu`) is exempt from the look-alike check, so `paypal.ευ` claims nothing

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has it
(scan.rs:287-298 `lookalike_ending`; endings.rs `is_bare`; tlds.txt:1388 `XN--QXA6A`).

TLDR: Link text `paypal.ευ` (Greek epsilon and upsilon), which reads as `paypal.eu`, on a link to another site shows no
warning. The same text with a Cyrillic `о` in a `.com` ending does warn.

Details: `lookalike_ending` ends with `&& !endings::a_label(last).as_deref().is_some_and(endings::is_real)`: an ending
that is itself a real top-level domain is never a look-alike, which is right for genuine internationalized endings
(`.рф`). But IANA's list includes `XN--QXA6A` (tlds.txt:1388), the Greek `ευ`, which a Latin reader takes for `eu`, a
bare ending here (two-letter, not in EXCLUDED). So `paypal.ευ`: `is_bare` fails (the A-label `xn--qxa6a` is not two
letters), `lookalike_ending` is suppressed by the real-ending exemption, and `qualify` requires a `/` after a real
ending (scan.rs:624). No Address, no warning. With a path (`paypal.ευ/login`) it is found as a plain address and warns
as a Mismatch against any other destination, but not as Disguised. SPEC.md (Terminal, OSC 8 paragraph) counts 'a dotted
host whose ending is a common site ending ... or looks like one with a non-ASCII letter'. Fix: before the real-ending
exemption, map the ending's look-alike letters to ASCII (a small Greek/Cyrillic-to-Latin skeleton) and treat the name as
disguised when the skeleton is a bare ending and the ending is not ASCII; or list such real IDN endings explicitly.
Verify by reading scan.rs:287-298 and tlds.txt for XN--QXA6A.

Suggested bucket: other. A real but narrow gap: it needs one specific Greek ending that only mimics `.eu`. Same
reasoning as the related f2: the hover always shows the real destination in full (here as punycode for an IDN
destination); if the maintainer treats a missed spoof warning as security-impacting in itself, it would rise with f2.

Confidence: confirmed (source and the snapshot establish the exemption and the TLD's presence; that `ευ` reads as `eu`
is a visual judgment).

proposed_drop: none.

possible_cover: f2 (same code, different trigger); a fix for f2 via a confusable skeleton would likely cover this if
applied before the real-ending exemption.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g01-scanner-src/f10`.
