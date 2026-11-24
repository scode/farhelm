# Restore mismatch warnings for quoted and bracketed terminal links

## What this was about

Hovering a terminal link shows its destination. After the earlier fix for false warnings on descriptive labels, an
address displayed entirely inside brackets or quotes could point to a different site without a mismatch warning. The
maintainer chose to recognize addresses inside matching pairs, including nested pairs.

## Things you should know

- The comparison checks the address inside angle brackets, parentheses, square brackets, and single or double quotes.
  When it warns, the hover still shows the original label beside the destination. Honest addresses and labels such as
  "See (https://example.test/)" get the ordinary destination preview.
- Review found that a bracketed address followed by a separate bracketed phrase could be mistaken for a single address.
  The comparison leaves a separate phrase after a closing delimiter in the label. Punctuation inside an address path
  still participates in the comparison.
- Two adjacent rows can each give a complete wrapped address. If different wrappers make their joined text lose the
  shape of an address, the hover compares the address on the row under the pointer. A joined label that has the shape of
  an address is compared as a whole.

## Open questions and possible follow-ups

None identified.

## PRs

- Restore the warning for addresses inside matching brackets and quotes.
  [#4746](https://github.com/scode/farhelm/pull/4746/changes)

## Checks run, reused and skipped

- The recorder ran pinned `cargo nextest run -p farhelm-grid-model --lib --test links` with four slots and zero retries
  at tested tree `092cf66f` (run `fc7d5381-b944-400d-995b-7bcf7575dc00`). All 30 tests passed, including the pinned link
  and disclosure fixtures. No runtime skips.
- `cargo clippy -p farhelm-grid-model --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and
  `git diff --check` passed at `092cf66f`.
- Reused `dprint check SPEC.md TODO.md releasing/changelog.d/osc8-link-disclosure.md`,
  `python3 releasing/check-changelog.py format`, and the isolated `python -B scripts/check-test-sleeps.py` from
  `89ea7d26`. The delay check inspected 310 delays and found none without a rationale. The final edit added one
  assertion to a pure unit test.
- The tested tree was used for the draft. Changes on main during validation only recorded other plans' claims. Skipped
  the workspace and desktop suites because the change is confined to the link comparison used by the grid model, which
  the focused integration tests exercise directly.

## Review gate

The source review passed. The reviewer confirmed both production fixes and asked for one stronger assertion to prove
that the comparison recognizes a URL with parentheses in its path. The final test run passed with that assertion. The
commit message passed a separate cold read.

### Landing

#### What landed with it

The plan's PR ([#4746](https://github.com/scode/farhelm/pull/4746/changes)) and one landing fix,
[#4748](https://github.com/scode/farhelm/pull/4748/changes), landed alone in their round. Nothing but plan bookkeeping
reached main while the plan was being built, so nothing could interact with it.

#### Fixes made while landing

- **A hidden character beside a bracket still stopped the warning.** The plan finds a bracket or quote pair after
  trimming only whitespace. The terminal keeps a zero-width character or combining mark with the cell before it, so a
  program can put one right after the closing `>` of `<https://good.test/>`, or between nested delimiters, while the
  screen shows a plain bracketed address. A blank filler character (one that takes a cell but draws nothing, such as
  U+3164) beside a bracket looks the same. Any of these meant the pair was not found, so a link to another site got only
  the plain destination preview. The landing fix finds the pairs past such characters. A hidden character between the
  innermost bracket and the address stays in the compared text and warns. This is the same kind of hole that the
  previous plan's landing (osc8-link-disclosure-fixes, #4739) closed for addresses shown without brackets.
- **A bracketed row beside a bare one was not compared row by row.** Take a hyperlink to `https://good.test/` spread
  over two rows, `<https://good.test/>` above `https://other.test/`. It was judged on the joined text, which starts with
  `<` and so is not an address, so the second row's claim to name `other.test` got no warning. The plan compared each
  row on its own only when every row was bracketed. The fix does it whenever every row shows an address, so hovering the
  second row warns. This can only add warnings.
- SPEC.md's description now says that a closing bracket or quote followed by more words makes the label descriptive
  text, and that the pairs are found past characters that draw nothing. Its sentence about rows that each show an
  address no longer uses "wrapped", which the same paragraph uses for soft wraps.

#### Checks

The plan changes only how the desktop terminal's link model decides whether a hyperlink's label claims a web address, so
the landing ran the tests of the link model, of the desktop GUI that shows its hover display and of the input mapping
code that also uses the model, clippy with warnings as errors on those, and the formatter and source-text checks listed
below. Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. No test run failed.
`cargo fmt --all -- --check` failed once after the landing fix, because the fix's code was not yet formatted; it was
formatted and the checks were run again.

On the plan's PR rebased onto main, before the landing fix:

- `cargo nextest run -p farhelm-grid-model -p farhelm-gui -p farhelm-input-mapping`, on the test worker (recorder run
  `8ce68808-ad65-4f5f-a2a8-b1a6afdbc8c1`): all 3452 passed. Nextest skipped four tests marked to run only on demand:
  three that compare the app's screenshots with stored reference pictures, and a helper that regenerates icon images.
- `cargo clippy -p farhelm-grid-model -p farhelm-gui -p farhelm-input-mapping --all-targets -- -D warnings`, on the test
  worker: passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fix (#4748):

- `cargo nextest run -p farhelm-grid-model`, locally (recorder run `accfa5a0-07b0-430f-ae13-0865b6c619e8`): all 71
  passed, including the new hidden-character and two-row tests and the fixed table of labels and expected warnings
  carried over from the old web client, which pins the warning's earlier behavior.
- `cargo nextest run -p farhelm-gui --lib` selecting the terminal hyperlink, printed path and file disclosure tests,
  locally (recorder run `2af7a5b3-0a7d-4e21-974d-b6e7664c6fa6`): all 47 passed.
- `cargo clippy -p farhelm-grid-model -p farhelm-gui --all-targets -- -D warnings`, the formatter, the sleep check,
  `dprint check` and the changelog format check, locally: passed. One code comment was reworded after these runs, and
  the formatter check was run again on it.

Reused for the fix: the rest of the GUI tests and the input mapping tests and lints above. The fix changes only how the
link model decides whether a label claims an address, inside the model, and nothing the model exposes to other code; the
model's own tests and the GUI tests of the hover display that uses that decision were rerun. Skipped: the reference
pictures, since nothing drawn changed and none of them shows a hyperlink with a bracketed or quoted label; the rest of
the workspace suite, since nothing outside the desktop GUI and its input mapping uses the link model; the end-to-end
desktop smoke, whose steps do not hover program hyperlinks; macOS, since the link model is the same code on every
platform and no Mac was used; and CI, which runs only on demand.

#### Things you should know that the report does not say

- The report says a separate phrase after a closing delimiter is kept in the label. In practice the label is peeled only
  when its first and last characters form a matching pair, and a label whose first word then ends in a closing bracket
  or quote, followed by more text, counts as descriptive text. It gets the ordinary display even when it starts with a
  familiar address that names another site: `[https://good.test/] [1]`, `(https://good.test/) :)`. The same text without
  brackets warns. SPEC.md now states the rule. Making such labels claim their leading address would also make common
  honest labels warn, such as `[https://good.test/] [docs]` on a link to a docs page on that site, and the ordinary
  display still names the real host. So I recommend leaving it unless such labels turn up as a real spoof.
- Some honest labels now warn where they did not before: two bracketed or quoted addresses separated by a comma, such as
  `"https://a.test/a", "https://a.test/b"`, or `(https://a.test/x), (more)`. The outer pair is peeled, and what is left
  starts like an address but is not the target. Such labels are rare, and the warning still names the real destination,
  so I suggest no follow-up.
- `'@good.test/'` on a link elsewhere warned before this plan, because the opening quote was read as a user name before
  the `@`. It now gets the ordinary display, the same as `@good.test/` without quotes already did. An address that
  starts with `@` is not a believable spelling of a familiar site, so losing the warning costs little, and I suggest no
  follow-up.

#### Possible follow-ups

- **A bracket printed outside the hyperlink.** If a program links only `<https://good.test/` and prints the closing `>`
  as plain text, the screen shows `<https://good.test/>` but the label has no matching pair, so nothing warns. This
  predates the plan and is outside what it set out to fix. Treating a label that opens with a bracket or quote and has
  no closer as if the closer were present would close it. I recommend that as a small follow-up. Unlike the
  phrase-after-a-bracket case above, an honest program has no reason to link a bracket without its closer, so it would
  add almost no false warnings.
