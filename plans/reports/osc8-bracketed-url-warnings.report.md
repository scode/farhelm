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
