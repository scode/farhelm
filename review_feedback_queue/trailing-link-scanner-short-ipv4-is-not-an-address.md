# A shortened IPv4 address with a port or path (`127.1:8080/admin`) is not treated as an address

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24

## TLDR

Link text like `127.1:8080/admin` names a local service to a browser (it opens 127.0.0.1:8080), yet the hover finds no
address in it and stays quiet when the link goes somewhere else entirely.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer's rule for warning bypasses (2026-11-24); the reviewer suggested `other`.

### Reviewer record (`g02-scanner-tests/f6`)

Location: `crates/gui/farhelm-grid-model/src/link_claims/tests.rs:247`. Summary: Shortened IPv4 with a port or path
(`127.1:8080/admin`) is pinned as no address, so it never warns

Revision: re-classified from candidate to finding at the orchestrator's request. My earlier basis ('an IPv4 address'
reads as dotted quad) is an interpretation, not explicit spec acceptance; the only explicit acceptance is a code comment
(scan.rs:622-623), which the brief says is not a candidate basis.

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has
it.

TLDR: Link text like `127.1:8080/admin` names a local service to a browser (it opens 127.0.0.1:8080), yet the hover
finds no address in it and stays quiet when the link goes somewhere else entirely.

Details: tests.rs:247 pins `("127.1:8080/", vec![])`. `qualify` (scan.rs:624-659) accepts an IP only when it has four
numeric labels (scan.rs:640-649); `127.1` has two labels whose ending `1` is not a real ending, so neither the bare nor
the path rule applies and no anchor qualifies. The URL parser the comparison uses reads `127.1` as 127.0.0.1, and the
same test file confirms that with a scheme: `https://127.1/` is found and matches `https://127.0.0.1/` (tests.rs ~1052,
~1077). So the scheme-less spelling names a different place than an arbitrary destination, and nothing in the code shows
the user is not misled; only the destination display remains. SPEC.md:3598 counts 'an IPv4 address or `localhost`
followed by a path or port', without restricting it to four octets, and the accepted-gap list does not name shortened
forms. Fix: accept 2- and 3-part all-numeric names (and other WHATWG IPv4 spellings, if wanted) when followed by a path
or port, as `localhost` is, and flip the test; or add the gap to SPEC.md. The practical spoof value is low (a shortened
loopback address rarely makes a link look trustworthy), which bears on priority, not on whether the spec holds.

Suggested bucket: other. Missed aid on an unusual spelling; the destination display is authoritative. Moves to highest
only if missed spoof warnings are counted as security-impacting.

Confidence: confirmed (source trace; not executed).

proposed_drop: none from the spec. The only acceptance is scan.rs:622-623 ('A bare `1.2.3.4` or `localhost` in prose
doesn't count, and neither does a shortened IPv4 like `127.1`'), a code comment, not SPEC.md/SPEC_impl.md.

possible_cover: none known.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g02-scanner-tests/f6`.
