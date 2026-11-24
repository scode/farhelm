# A ported scanner test's `\u{200b}https://` case no longer tests what it claims

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24

## TLDR

A test meant to guard against a disguised scheme being quietly read as a site name has a case that no longer exercises
that, so a future change that wrongly borrowed the link's scheme for such text would still pass this case. No
user-visible effect today.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it.

### Reviewer record (`g02-scanner-tests/f4`)

Location: `crates/gui/farhelm-grid-model/src/link_claims/tests.rs:1245`. Summary: Ported test's `\u{200b}https://` case
no longer tests scheme reinterpretation; it passes only because its target is malformed

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24 (ported the case); ba0da50536236ca934f3956d67a0c35e4b57586e
re-labelled the section 'identical assertions unless a case says why' without touching it. The tip
e159ffc79ab147be44e92a92b59571650a4f322d still has it.

TLDR: A test meant to guard against a disguised scheme being quietly read as a site name has a case that no longer
exercises that, so a future change that wrongly borrowed the link's scheme for such text would still pass this case. No
user-visible effect today.

Details: `a_lookalike_scheme_is_not_reinterpreted_as_a_hostname` (tests.rs:1239-1252) builds
`target = Url::parse(format!("https://{label}"))` and asserts `Url::parse(label).is_err()` and
`mismatch(label, target)`. Its docstring: 'Borrowing the destination's scheme could produce a valid URL from an address
whose original scheme cannot be parsed.' For the second label, `\u{200b}https://example.test/x`, the new scanner puts
the zero-width space in its own grapheme cluster that folds to nothing, so the scheme anchor (scan.rs:396-434) starts
the address at `h` and the address is `https://example.test/x`, which parses fine; the ZWSP is not part of any compared
text (ba0da505's own `characters_that_draw_nothing_beside_a_delimiter...` docstring at tests.rs ~1181-1188 says exactly
this for `<\u{200b}https://...>`). The `Url::parse(label).is_err()` assertion is about the whole label, not the address,
and `mismatch` holds only because the constructed target is `https://<ZWSP>https://example.test/x`, i.e. host `https`
with path `//example.test/x`, which differs from any honest reading. So the case asserts nothing about reinterpretation.
The first case (`htt\u{440}s://`) still does. Fix: drop the second case (its behavior is now covered by the bracket test
and SPEC.md's accepted gap 'an invisible character before an address whose visible text is the target itself is not a
disguise'), or replace it with one whose scheme itself holds the invisible character (`ht\u{200b}tps://example.test/x`
against a target that borrowing would produce), and note the change as the section header now requires.

Suggested bucket: other. Test quality only.

Confidence: confirmed (source trace; not executed).

proposed_drop: none.

possible_cover: none.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g02-scanner-tests/f4`.
