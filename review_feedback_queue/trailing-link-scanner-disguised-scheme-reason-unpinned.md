# No scanner test pins the look-alike reason for a disguised scheme

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24

## TLDR

If a later change stopped noticing invisible or look-alike characters in the `https` part of link text, the hover would
still warn, but with the generic 'mentions an address that differs' line instead of 'uses look-alike or invisible
characters', and no test would fail.

## Details

**other.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it.

### Reviewer record (`g02-scanner-tests/f5`)

Location: `crates/gui/farhelm-grid-model/src/link_claims/tests.rs:1167`. Summary: No test pins the 'look-alike' reason
for a disguised scheme; the scheme cases only assert 'some warning'

Reviewed commit: 18845174e7599d1def2461bccb38b94a175c5b24; the tip e159ffc79ab147be44e92a92b59571650a4f322d still has
it.

TLDR: If a later change stopped noticing invisible or look-alike characters in the `https` part of link text, the hover
would still warn, but with the generic 'mentions an address that differs' line instead of 'uses look-alike or invisible
characters', and no test would fail.

Details: `example_labels_get_these_verdicts` (tests.rs:571ff) asserts reasons and its docstring says the reason is
asserted 'because the hover's wording comes from it: a disguise reported as a plain mismatch hides why the link is
dangerous'. Its Disguised rows cover only the host (`exa\u{feff}mple.com`, `\u{ff45}xample.com`), a look-alike ending,
and the authority end (`github.com\u{2215}login`, tests.rs:1047). Every scheme-disguise label (`ht\u{200b}tps://`,
`http\u{34f}s://`, `http\u{fe0f}s://`, `https\u{2d0}//`, tests.rs:1150-1176) goes through `mismatch(...)`, a boolean,
against `https://other.test/x`. For those, a written scheme that recognition changed never parses (`compare::parse` uses
`Url::parse` on the original text), so the boolean holds whether or not `accept` (scan.rs:728-743, judged=true for
scheme clusters) marks the form disguised; a regression in that marking would silently turn Disguised into Mismatch. The
same gap exists in `tests/links.rs`, whose Disguised rows are also host-only (links.rs:1432-1455, 1515-1523). Note also
that several 'still warn' cases in `disguised_addresses_still_warn` and
`invisible_characters_inside_an_address_still_warn` would pass with no folding at all (e.g. `exam\u{200b}ple.test/x`
unfolded still yields `ple.test/x`, which differs from other.test), so those tables prove recognition less than their
docstrings suggest. Fix: add rows like `("ht\u{200b}tps://example.test/x", "https://example.test/x", Some(Disguised))`
and `("https\u{2d0}//example.test/x", "https://example.test/x", Some(Disguised))` to the verdict table, and same-target
rows (`exam\u{200b}ple.test/x` against `https://example.test/x` -> Disguised) for the host cases in the ported tables.

Suggested bucket: other. Missing test coverage; current behavior is correct by source reading.

Confidence: confirmed (source reading of tests and scanner; not executed).

proposed_drop: none.

possible_cover: none.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g02-scanner-tests/f5`.
