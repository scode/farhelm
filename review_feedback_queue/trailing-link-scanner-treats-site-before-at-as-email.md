# `paypal.com@evil.test` is read as an email address and never warns

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24

## TLDR

Link text such as `paypal.com@evil.test` or `accounts.google.com@login.test`, on a link that goes somewhere else, shows
no warning, because the text is read as an email address. A reader may take in the familiar site name at the front.
Adding a path, query, fragment or port after it (`paypal.com@evil.test/x`) makes it warn as login details before a host,
and so does a `www.` prefix.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer (2026-11-24). Both reviewers proposed dropping it as spec-accepted
(SPEC.md: "Email addresses ... claim nothing"); the independent drop check DISAGREED: the same SPEC.md sentence counts a
host after login details and an `@` as an address that always warns, and only the code's `www.` carve-out
(scan.rs:222-230) draws the line. The checker suggests SPEC.md say where an email ends and login details begin either
way.

### Reviewer record (`g01-scanner-src/f6`)

Location: `crates/gui/farhelm-grid-model/src/link_claims/scan.rs:231`. Summary: email-shaped text `paypal.com@evil.test`
(no path or port) claims nothing, so it never warns

Revision of candidate f6 into a finding with a proposed drop, because a spoof-warning gap could be in `highest`
(orchestrator question, turn 2).

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has it
(scan.rs:231 `suppressed`, CLAIMLESS regex above it).

TLDR: Link text such as `paypal.com@evil.test` or `accounts.google.com@login.test`, on a link that goes somewhere else,
shows no warning, because the text is read as an email address. A reader may take in the familiar site name at the
front. Adding a path, query, fragment or port after it (`paypal.com@evil.test/x`) makes it warn as login details before
a host, and so does a `www.` prefix.

Details: `suppressed` (scan.rs:231) records an email span for any `[A-Za-z0-9._%+-]+@name.name` match not followed by
`/`, `?`, `#` or `:` and a digit, unless the local part starts with `www.`; anchors inside the span are skipped
(`Scanner::run`), so the dotted name `paypal.com` in the local part and the `@` anchor are both hidden. Verify with
tests.rs ~446 (`www.paypal.com@evil.test` is the only email-shaped spoof the tests treat as an address). A narrower
email reading would exempt local parts whose last dot-label is a bare ending (`com`, `org`, ...), since real email local
parts rarely end in `.com`, treating those like the `www.` case.

Suggested bucket: other if the spec's acceptance stands; otherwise high. The hover always shows the real destination in
full with its host emphasized, which is why it is not `highest`; if the maintainer treats a missed spoof warning as
security-impacting in itself, `highest`.

Confidence: confirmed (source establishes the suppression).

proposed_drop: SPEC.md, Terminal section, OSC 8 paragraph: "Email addresses, handles (`@name`) and SCP-style git remotes
(`git@github.com:org/repo.git`) claim nothing, and neither do file names, version numbers or dotted code names." The
text is syntactically an email, so the spec accepts the silence.

possible_cover: none known.

### Also reported by `g02-scanner-tests/f2`

Location: `crates/gui/farhelm-grid-model/src/link_claims/tests.rs:234`. Summary: Tests pin `github.com@evil.com` as a
claimless email, so a site name used as userinfo without a path never warns

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has
it.

TLDR: Link text such as `paypal.com@evil.com` is the classic 'familiar name in the login details' trick, but the hover
treats it as an email address and says nothing, whatever the link really opens. `www.paypal.com@evil.com`,
`paypal.com:x@evil.com` and `paypal.com@evil.com/login` all warn, so the quiet hinges on leaving off `www.`, a colon or
a path.

Details: tests.rs:234 lists `("github.com@evil.com", vec![])` under 'Emails, a mailto, a handle and an SCP remote claim
nothing'. In `suppressed()` (scan.rs:231-275) the email alternative of `CLAIMLESS` (scan.rs:46-51) matches the whole
token, the local part does not start with `www.` and nothing follows, so the span is suppressed and `Scanner::run`
(scan.rs:339-345) skips it whole: no address, no mismatch, against any target. The `www.` carve-out (scan.rs:224-226,
256-260, doc: '`www.paypal.com@evil.test` is a site name in front of another host') shows the authors treat a site name
before `@` as userinfo, but only when spelled with `www.`; a local part that itself qualifies as a bare host
(`paypal.com`, ending in a bare ending) is equally a site name. SPEC.md counts 'a host after login details and an `@`'
as an address and says 'one with login details before its host, always warns', while also saying 'Email addresses ...
claim nothing'; which reading `paypal.com@evil.com` gets is not decided by the spec, and the test picks the quiet one.
Fix options: extend the `www.` carve-out to any local part whose last dotted label is a bare ending (real email local
parts rarely end in `.com`/`.io`/a country code), add a test pinning the chosen reading with its rationale, or list the
case in SPEC.md's accepted gaps.

Suggested bucket: other. The destination display still names the real host; harm needs the user to misread a correct
display. Could move to highest only if the maintainer counts a missed phishing warning as security-impacting.

Confidence: confirmed (source trace; not executed).

proposed_drop: SPEC.md, the OSC 8 hover paragraph: 'Email addresses, handles (`@name`) and SCP-style git remotes
(`git@github.com:org/repo.git`) claim nothing' — `github.com@evil.com` is syntactically an email, so the maintainer may
consider this accepted.

possible_cover: none known. Overlaps g01-scanner-src.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g01-scanner-src/f6`, `g02-scanner-tests/f2`.
