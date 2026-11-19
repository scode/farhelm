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

### Landing

#### What landed with it

It landed in a round of nine plans: session-group-follow-ups, model-families, session-limit-notice,
reserved-resource-names, secrets-logs-and-screen, checkout-settings-stale-save, concealed-link-target, triage-misc-nov18
and triage-discards-nov18. Since its base, main gained the previous round's plans (submenus that open on hover, Clone
keeping invisible characters, the retired old-terminal announcement, quick switcher screenshot folders, the update
survival wording) and docs screenshot tooling. triage-discards-nov18, in this round, discards a finding about selections
copying concealed text; that agrees with this plan's decision to leave selection alone. The reviews found no other
interaction.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The fix is correct and small:
concealed text is treated as spaces only when looking for printed links, so no hidden character can become part of an
opened address, and both new tests would fail without it.

#### Fixes made while landing

- **A code comment lost a detail and gave a backwards reason.** It dropped how much text around the click is searched
  for links (2048 characters), and its new reason ("because a plain click needs no destination preview") had the logic
  upside down: the click opens without showing the address first, so it may open only what the terminal draws. Both are
  fixed.

#### Things you should know that the report does not say

- The SPEC.md addition is three short sentences where one would do. I left it.

#### Checks

This round landed nine plans together, so these checks cover all nine on one tree: the nine stacks one after another on
main. Between them the plans change the desktop app's GUI crates, the helm and its client, the shared protocol, the
supervisor, the desktop process, SPEC.md, SPEC_impl.md and the triage records. The landing ran the lints, the whole Rust
workspace suite, the desktop process's own tests, the app's end-to-end smoke and the reference screenshots (goldens).
Builds ran locally; the workspace suite and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, `python3 -B scripts/test-gui-e2e-smoke.py`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`, once without and once with `--features agent`: passed.
- `dprint check`: failed on `TRIAGE_OUTCOMES_ARCHIVE.md`, which triage-misc-nov18 wrote unformatted. Fixed.
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats:
  2744, 2653 and 2625 tests, 8022 in all (`1ffb9472-9f12-4890-b0d5-032dd87718d0`,
  `7c3ef63b-2e2b-4903-acff-22cbe9409601`, `7c539d75-d409-48c7-901b-391169215c11`). One failed: the macro
  compile-contract test, which builds a small crate offline from this machine's cargo cache, which a boat does not have,
  so it fails there every time; it passed locally (below). No runtime skips.
- `cargo nextest run -p farhelm-desktop`, locally: 176 passed (`59017e5e-7626-4dca-9c45-0b4972a89aca`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed, the new step that
  drags an unpinned session into a group included, and the clipboard paste step was skipped, as always on Linux
  (`b1164f8b-3d55-4cac-9fb1-a76e9a8da60c`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`da0dca91-646f-483a-a436-b10b43ffafe2`), the new groups
  pictures included.

On the fixed tree (comments, SPEC.md, SPEC_impl.md, changelog fragments, the triage archive's formatting, and one
notice's wording changed):

- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`: passed.
- The bell notices' tests in the GUI and the helm, locally: 26 passed (`80fb276d-1ba3-4b09-a84a-370aef2b9605`).
- `cargo nextest run -p farhelm-testtrace-macros`, locally: 2 passed, the compile-contract test included
  (`a8a9bf1f-6dc9-4e24-8831-e5d4b7f5dbc0`).

The boat suites, the smoke and the goldens were reused for the fixed tree: after them only comments, documents,
changelog fragments, formatting and one notice's wording changed, which the focused runs above cover.

Skipped:

- The Linux app lifecycle: startup and quit are untouched.
- The Mac-only checks: no Mac-specific code changed.
