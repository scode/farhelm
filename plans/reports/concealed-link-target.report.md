## What this was about

A program could hide part of a URL printed in the session terminal. Clicking the visible text could open a different
address because Farhelm included the hidden characters. The maintainer chose to have concealed text break printed links
while keeping text selection unchanged.

## Things you should know

A printed link now ends at the first explicitly concealed cell. A click can still open the visible prefix when it forms
a valid URL. The visible text after a concealed run is scanned separately. Ordinary visible links and links the program
explicitly marks as hyperlinks keep their behavior. Copying still includes any concealed characters within the selected
range.

## Open questions and possible follow-ups

The fix covers the terminal's conceal attribute. Text drawn in its background color can still be part of a link, as
agreed during triage.

## Pull requests

- Split printed links at concealed cells and archive the completed triage item.
  [#4154](https://github.com/scode/farhelm/pull/4154/changes)

## Checks

- All 42 `farhelm-grid-model` tests passed locally on the tree captured at `dbc9ecdc` using
  `cargo nextest run --locked -p farhelm-grid-model` (run `da02e2ae-308f-4e66-adf0-71f75300f334`). The new tests cover a
  hidden suffix and a concealed section after a soft wrap. After changing the selection test to copy the range from a
  double-click, all seven link tests passed again on `faff697b` using
  `cargo nextest run --locked -p farhelm-grid-model --test links` (run `7c4f9302-7db3-4cfa-9713-b9fb68e08f62`). The
  later ledger edit only adds the draft URL. Existing tests cover ordinary URLs, hyperlink precedence, and cell
  ownership.
- `cargo clippy --locked -p farhelm-grid-model --all-targets -- -D warnings`, Rust and Markdown formatting, changelog
  format, whitespace, instruction links and test-delay checks passed. The instruction check found a pre-existing archive
  citation warning.
- The change is in the terminal's grid model, with local coverage for URL recognition and selection. I skipped the
  desktop smoke, workspace suite and hosted CI because the app's click path and protocol were unchanged.

## Review gate

The independent review found no product correctness defects. I made the regression check go through the actual
double-click selection range and clarified the changelog wording. The commit title and body also passed a separate cold
read.
