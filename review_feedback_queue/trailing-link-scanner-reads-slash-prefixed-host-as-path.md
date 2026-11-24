# A familiar host after a single leading `/` (`/paypal.com/login`) is read as a file path

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24

## TLDR

Link text such as `/paypal.com/login` or `/github.com/org/repo` reads to many people as a site and path, but the hover
treats it as a file path, finds no address in it, and gives no warning whatever the link actually opens.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer (2026-11-24). The reviewer proposed dropping it as spec-accepted
("neither do file names"); the independent drop check DISAGREED: that sentence names file names, not paths whose first
part is a site, SPEC.md counts an address glued to punctuation, and the accepted-gap list names the `_`-glued case but
not this one.

### Reviewer record (`g02-scanner-tests/f8`)

Location: `crates/gui/farhelm-grid-model/src/link_claims/tests.rs:411`. Summary: A familiar host after a leading `/`
(`/paypal.com/login`) is read as a file path and never warns

Revision: re-classified from candidate to finding at the orchestrator's request, because a missed spoof warning could be
`highest` and the spec acceptance below is an interpretation, not an explicit gap entry.

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has
it.

TLDR: Link text such as `/paypal.com/login` or `/github.com/org/repo` reads to many people as a site and path, but the
hover treats it as a file path, finds no address in it, and gives no warning whatever the link actually opens.

Details: `host_may_start` (scan.rs:587-615) refuses a host whose preceding character is a barrier, and `/` is a barrier
(scan.rs:588-589) in both the original and folded readings. The protocol-relative anchor `slashes` (scan.rs:482-497)
needs `//`, so a single leading `/` yields nothing. For `/paypal.com/login` against `https://evil.test/`, no anchor
qualifies, `judge` returns no addresses and no mismatch. The tests pin the rule with `/tmp/example.test/x` and
`src/docs.rs/x` (tests.rs:411 and :930, 'a path'), not with a familiar host directly after the first `/`. Not refuted
from code: the text can name a place (paypal.com/login) other than the destination, and nothing shows the user is not
misled except the always-present destination display. A narrower rule, e.g. allowing a host right after a single leading
`/` at the token start when its ending is bare (`/paypal.com/...`, but not `/tmp/example.test/x`, whose first segment
has no bare ending), would keep the file-path cases quiet; it would need tests both ways.

Suggested bucket: other. The destination display always shows the real host; harm requires misreading that correct
display (bucket rule of 2026-10-22). Moves to highest only if the maintainer counts a missed spoof warning as
security-impacting; surface at triage.

Confidence: confirmed (source trace; not executed).

proposed_drop: SPEC.md:3599-3601, the OSC 8 hover paragraph: 'Email addresses, handles (`@name`) and SCP-style git
remotes (`git@github.com:org/repo.git`) claim nothing, and neither do file names, version numbers or dotted code names.'
A `/`-led dotted path can be read as a file name under that sentence; the accepted-gap list (SPEC.md ~3638-3647) does
not name it explicitly.

possible_cover: none known.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g02-scanner-tests/f8`.
