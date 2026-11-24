# A scheme-less host glued to Chinese or Japanese prose is read as part of the host

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24

## TLDR

In Chinese or Japanese link text, which puts no space before a site name, a site written without `https://` is read
together with the preceding words. An honest link whose text says 'see github.com/x' in Chinese, pointing at
github.com/x, warns that the text mentions a different address; and text that says 'please see github.com. thanks'
mentions no address at all, so it stays quiet whatever the link opens.

## Details

**high.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket `high` per the maintainer's rule for warning bypasses (2026-11-24); the reviewer suggested `other`.

### Reviewer record (`g02-scanner-tests/f10`)

Location: `crates/gui/farhelm-grid-model/src/link_claims/scan.rs:40`. Summary: Scheme-less host glued to CJK prose is
read as part of the host: honest `详见github.com/x` warns, `请看github.com。谢谢` is missed; untested

Revision: confidence raised from likely to confirmed (reason under Confidence); test line references corrected.

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has
it.

TLDR: In Chinese or Japanese link text, which puts no space before a site name, a site written without `https://` is
read together with the preceding words. An honest link whose text says 'see github.com/x' in Chinese, pointing at
github.com/x, warns that the text mentions a different address; and text that says 'please see github.com. thanks'
mentions no address at all, so it stays quiet whatever the link opens.

Details: The scheme anchor handles CJK prose explicitly (scan.rs:390-393, 410-420: 'Where a letter of another script
than Latin, Greek or Cyrillic runs straight into ASCII letters, the scheme starts at the last such change', tested at
tests.rs:134, :318 and :611 with `\u{8be6}\u{89c1}https://...`). The host anchor has no equivalent: `NAME`
(scan.rs:40-41) takes `[\p{L}\p{M}\p{N}-]+` labels, so CJK letters join the first label. Trace for
`\u{8be6}\u{89c1}github.com/x` against `https://github.com/x`: the one name is `详见github.com`, `host`
(scan.rs:556-573) anchors at the token start, `qualify` (scan.rs:624-659) accepts the bare ending `com`, `accept` makes
host `详见github.com`; `compare::parse` gives an IDN host (`xn--...`), `comparable` differs, so `judge` reports Mismatch
and the hover quotes `详见github.com/x`. Trace for `\u{8bf7}\u{770b}github.com\u{3002}\u{8c22}\u{8c22}` against
`https://evil.test/`: the recognition view folds `。` to `.`, so the one name is `请看github.com.谢谢`, whose last label
`谢谢` is neither real nor bare nor a look-alike ending (CJK letters are not confusable, scan.rs:307-314); there is no
path, so no anchor qualifies and nothing is reported. SPEC.md says 'An address counts anywhere in the text: in prose,
...' and does not list this in its accepted gaps. Tests: `only_confusable_scripts_make_a_look_alike_ending` (tests.rs
~782ff) shows the authors considered CJK no-space prose with `。`, but only for text with no site in it; no test covers
a scheme-less site glued to CJK prose in either direction. Fix sketch: start a host's first label at the last
other-script-to-ASCII change, as the scheme anchor does (and stop a name at the reverse change before an ending when the
remainder is CJK), then add tests for both directions.

Suggested bucket: other. The false alarm is confined to CJK labels with scheme-less sites, and the missed case still
leaves the authoritative destination display; could be `high` if CJK-language program output with such labels proves
common.

Confidence: confirmed (source trace; not executed). Both outcomes of the earlier open premise warn: if the URL parser
accepts the mixed CJK/Latin label it yields an IDN host other than github.com, and if it rejects it, compare.rs:64-66
treats the unparseable address as a mismatch. So the false alarm does not depend on IDNA behavior.

proposed_drop: none.

possible_cover: none known. Primarily g01-scanner-src scope; recorded here as a test gap.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g02-scanner-tests/f10`.
