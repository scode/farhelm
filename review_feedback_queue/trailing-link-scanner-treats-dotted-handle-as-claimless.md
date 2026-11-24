# A handle-shaped `@paypal.com` with nothing after it claims nothing

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24

## TLDR

Link text `@paypal.com` (or `Follow @github.com`) on a link to another site shows no warning: a handle whose name is a
dotted site name is treated as a social handle, never as the site it spells.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer (2026-11-24). The reviewer proposed dropping it as spec-accepted
("handles (`@name`) ... claim nothing"); the independent drop check was AMBIGUOUS: SPEC.md never says a handle's name
may be a dotted site name, and other SPEC.md lines count a dotted name on a common ending as an address even glued to
punctuation.

### Reviewer record (`g01-scanner-src/f12`)

Location: `crates/gui/farhelm-grid-model/src/link_claims/scan.rs:231`. Summary: handle-shaped `@paypal.com` (dotted
name, nothing after) claims nothing, so it never warns

Recorded as a finding with a proposed drop because, like f6, a spec-accepted spoof-warning gap could be in `highest`.

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has it
(CLAIMLESS handle branch above scan.rs:231, `suppressed`; tests.rs:236 asserts `@nytimes.com` finds nothing).

TLDR: Link text `@paypal.com` (or `Follow @github.com`) on a link to another site shows no warning: a handle whose name
is a dotted site name is treated as a social handle, never as the site it spells.

Details: CLAIMLESS's handle alternative `@[\p{L}\p{M}\p{N}_.-]+(...)?` takes dots, and `suppressed` records the span
when the `@` starts the token or follows a non-local character and no `/`, `?`, `#` or `:digit` follows, so `paypal.com`
inside it never anchors. Distinct from f5, which is the path-followed shape the spec does not exempt. A narrower rule
would treat a handle whose name ends in a bare ending (`.com`, `.org`, ...) as also naming that site, at the cost of
warning on Bluesky/Mastodon-style domain handles (`@nytimes.com`) whose link goes to the profile on another host.

Suggested bucket: other if the spec's acceptance stands; otherwise high. The hover always shows the real destination in
full with its host emphasized, which is why it is not `highest`; if the maintainer treats a missed spoof warning as
security-impacting in itself, `highest`.

Confidence: confirmed (source and test).

proposed_drop: SPEC.md, Terminal section, OSC 8 paragraph: "Email addresses, handles (`@name`) and SCP-style git remotes
(`git@github.com:org/repo.git`) claim nothing". The scanner's doc (scan.rs CLAIMLESS) states the dotted case on purpose:
'A handle's name takes dots, so the whole handle is matched (`@nytimes.com`)'.

possible_cover: none known.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g01-scanner-src/f12`.
