## What this was about

A checkout editor left open could undo a newer change made from the command line. Saving a post-clone command could
replace the newer command that would run in future checkouts. On a host, Use default could also clear a newer folder
override. The triage decision was to refuse stale writes for these settings and keep last-write-wins for launch
templates and number-key bindings.

## Things you should know

Saving the default folder, a host folder or the post-clone command now checks the specific setting the editor loaded.
Use default checks the stored host override. A refused save discards the edit, gives its reason and loads the current
value before another save is available. An unrelated checkout setting can change without blocking the save. If inline
launcher setup discovers a new folder on that read, it refreshes the repository suggestions and destination preview and
keeps the refusal visible.

The new desktop app and its embedded helm check saves together. Either combination with an older app or older helm uses
the original unconditional save. Tests checked the old request on the new helm's route and the new app's request after a
test helm supplied the old reply shape. They exercised both sides of the exchange through fixtures.

## Open questions and possible follow-ups

None.

## Pull requests

- Protect command-line edits to checkout settings and archive the completed triage outcome.
  [#4160](https://github.com/scode/farhelm/pull/4160/changes)

## Checks

- All 309 selected Linux tests passed with no runtime skips in recorder run `7c820e0c-1ca1-42cd-90ca-0fdcb7a110f8`. The
  command after the recorder's `--` was
  `cargo nextest run -p farhelm-helm -p farhelm-gui --lib -E 'test(/^checkout_config::|^drafts::|^settings::|^hosts::settings::tests::|^launcher::checkout_tests::/)'`.
  It covered matching and stale writes, Use default, distinct unset and disabled command values, and launcher recovery
  without reopening.
- `cargo clippy -p farhelm-helm -p farhelm-gui --all-targets -- -D warnings`, Rust and Markdown formatting, changelog
  format, test-delay and instruction checks, and `git diff --check` passed. The instruction check reported one existing
  archive citation warning. The recorded test tree was based on `f69fd487` with the reviewed patch. The rebase to
  `b10c2e93` kept that patch byte for byte and picked up only plan queue and report edits. The draft's later archive
  update adds its PR link, so the runtime results still apply.
- The focused tests cover the changed helm transactions and GUI views, including the text of the launcher warning. I
  skipped the workspace suite and hosted CI because these paths have targeted local coverage. The warning reuses the
  launcher's existing alert style, so I used its view test for the new state and skipped the full screenshot suite. This
  run exercised Linux. It had no native macOS environment.

## Review gate

The independent review found that an old Use default button could clear an override before the window drew it and that
the launcher could keep its failed preview after finding the new folder. The fix ties each button to the read it was
drawn from and uses the launcher's existing refresh after discovering a changed folder. The review also identified
outdated spec wording and a draft accessor that needed its explicit-load requirement documented. The commit message
passed a separate cold read.

The reviewer verified all four findings were resolved in the final draft at `a53a964e`.

### Landing

#### What landed with it

It landed in a round of nine plans: session-group-follow-ups, model-families, session-limit-notice,
reserved-resource-names, secrets-logs-and-screen, checkout-settings-stale-save, concealed-link-target, triage-misc-nov18
and triage-discards-nov18. Since its base, main gained the previous round's plans (submenus that open on hover, Clone
keeping invisible characters, the retired old-terminal announcement, quick switcher screenshot folders, the update
survival wording) and docs screenshot tooling. secrets-logs-and-screen, in this round, also changed the shared
text-field draft this plan builds on; I merged the two by keeping both, as that plan's notes describe. The compatibility
with an older app or helm that the report describes works in both directions, and the reviewers found no problem in it.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. No part of it is bigger than
the plan needed.

#### Fixes made while landing

- **The changelog fragment lacked its caveat.** A refused save now discards the typed edit and shows the stored value
  again. That includes a conflict, where a long post-clone command must be typed again, and a refusal for an invalid
  value such as a relative folder, which used to keep the text for correction. Both follow SPEC.md's rule, but users
  will notice, so the fragment now says so for the release notes.
- **A code comment at the extension point was stale.** The shared draft's comment still said compare-on-save was not
  built. It now names the three new operations, what each is for, and points to the checkout settings code that uses
  them.

#### Things you should know that the report does not say

- After a refusal in the launcher's inline setup, once the refresh has removed the setup field, the launcher keeps
  saying "Review the current value before saving again" with nothing left to review or save. SPEC.md now requires the
  refusal to stay visible, so changing this means adjusting that sentence too. Possible follow-up: drop the message once
  the field it refers to is gone.

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
