# A host spelled entirely in look-alike letters is not treated as an address

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24

## TLDR

A program can underline text that reads exactly like `paypal.com` or `apple.com`, written entirely in Cyrillic
look-alike letters (`раураӏ.сом`, `аррӏе.сом`), on a link that goes to an attacker's site, and the hover shows no
warning. The same spoof with even one ASCII letter mixed in does warn. The product spec promises that a dotted host
whose ending 'looks like' a common site ending with a non-ASCII letter counts as an address, and its list of known gaps
does not mention this one.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer (2026-11-24): a warning bypass where the hover still shows the full real
destination.

### Reviewer record (`g01-scanner-src/f2`)

Location: `crates/gui/farhelm-grid-model/src/link_claims/scan.rs:287`. Summary: all-look-alike host such as Cyrillic
`раураӏ.сом` claims nothing, contrary to SPEC.md and unlisted among its caveats

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24 (introduced lookalike_ending); the tip
e159ffc79ab147be44e92a92b59571650a4f322d still has it unchanged.

TLDR: A program can underline text that reads exactly like `paypal.com` or `apple.com`, written entirely in Cyrillic
look-alike letters (`раураӏ.сом`, `аррӏе.сом`), on a link that goes to an attacker's site, and the hover shows no
warning. The same spoof with even one ASCII letter mixed in does warn. The product spec promises that a dotted host
whose ending 'looks like' a common site ending with a non-ASCII letter counts as an address, and its list of known gaps
does not mention this one.

Details: scan.rs:287-298 `lookalike_ending` requires `name.chars().any(|ch| ch.is_ascii_alphabetic())`, so a name with
no ASCII letter is never a look-alike ending. `сом` is not a real ending (not in tlds.txt), so `qualify` does not admit
it as a bare or path-followed host either (scan.rs ~620-660, 829). Result: `раураӏ.сом` (and `раураӏ.сом/login`, since a
path only qualifies a real ending) produce no Address, so `judge` returns no mismatch. The code comment at
scan.rs:283-286 calls this an 'Accepted gap', motivated by Russian/Greek prose abbreviations (`т.е.`, `π.χ.`), but
SPEC.md (Terminal section, the OSC 8 paragraph) says an address is 'a dotted host whose ending is a common site ending
... or looks like one with a non-ASCII letter', and its 'The warning is an aid, not a guarantee:' list enumerates
accepted gaps (docs.rs, secure_paypal.com, git remotes, www.intranet/x, ...) without this one. Either the spec should
list it as an accepted gap (and SPEC.md's definition be narrowed), or the check should accept an all-look-alike name
when the ending folds to a bare ending through a confusable skeleton (e.g. compare the ending against ALWAYS_BARE after
mapping Cyrillic/Greek look-alikes to ASCII), which would not hit `т.е.` (one letter) or `π.χ.` (one letter per label)
since the >=2-letter rule already excludes them. Verify by reading scan.rs:287-298 and tests.rs:864, whose case
deliberately keeps an ASCII `l`.

Suggested bucket: high. It is a straightforward bypass of the link-spoofing warning with a classic homograph; it is not
`highest` only because the hover still always shows the real target in full with its host emphasized (SPEC.md: 'the
ordinary display, which always names the real host, is what remains'); if the maintainer treats a missed warning on a
homograph as security-impacting in itself, it belongs in `highest`.

Confidence: confirmed (source establishes the no-ASCII-letter condition and that `сом` is not a real ending).

proposed_drop: none. The code comment calls it an accepted gap, but SPEC.md does not accept it: its definition includes
endings that look like a common one with a non-ASCII letter, and its gap list omits it.

possible_cover: none known.

### Also reported by `g02-scanner-tests/f3`

Location: `crates/gui/farhelm-grid-model/src/link_claims/tests.rs:855`. Summary: A host spelled entirely in look-alike
letters (`раураӏ.сом`) is no address, contrary to SPEC.md; tests pin the narrower rule

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has
it.

TLDR: Link text that spells `paypal.com` wholly in Cyrillic look-alike letters (no plain Latin letter anywhere) is not
recognized as an address, so the hover gives no 'look-alike characters' warning, although the product spec says an
ending that looks like a common one with a non-ASCII letter counts. Mixing in one plain Latin letter does warn.

Details: SPEC.md (OSC 8 hover paragraph) defines an address as including 'a dotted host whose ending is a common site
ending (...) or looks like one with a non-ASCII letter', and its accepted-gap list does not mention fully look-alike
names. `lookalike_ending` (scan.rs:287-298) additionally requires `name.chars().any(|ch| ch.is_ascii_alphabetic())`; its
doc (scan.rs:284-286) calls the all-look-alike spoof an 'Accepted gap', but that acceptance lives only in a code
comment. For `\u{440}\u{430}\u{443}\u{440}\u{430}\u{4cf}.\u{441}\u{43e}\u{43c}` the ending `сом` is not a real or bare
ending (its A-label is not in the root zone), `lookalike_ending` is false, `qualify` (scan.rs:624-659) rejects it, and
no other anchor applies, so `judge` finds nothing and reports no mismatch against any target. In the tests,
`prose_abbreviations_are_not_look_alike_endings` (tests.rs:855ff) states the ASCII-mix signature as the rule and
`accented_latin_letters_do_not_make_a_look_alike_ending` (tests.rs:883ff) and this test only exercise `раураl.сом` with
an ASCII `l`; no test pins (or flags) the all-look-alike case, so the spec contradiction is invisible to the suite. Fix:
either add the gap to SPEC.md's accepted-gap list (with the prose-abbreviation rationale), or widen the rule (e.g.
accept a last label of 2+ confusable letters that NFKC/skeleton-maps to a bare ending such as `com`), and add a test
either way.

Suggested bucket: other. The destination display still shows the real host (as punycode for an IDN); this is a missed
aid, and a spec/code contradiction. Could move to highest only if the maintainer counts a missed homograph warning as
security-impacting.

Confidence: confirmed (source trace; not executed). Revision: corrected a test name in Details.

proposed_drop: none (the acceptance is in scan.rs's doc comment, not in SPEC.md/SPEC_impl.md).

possible_cover: none known. Primarily g01-scanner-src scope; recorded here because the tests' docstring pins the
narrower rule.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g01-scanner-src/f2`, `g02-scanner-tests/f3`.
