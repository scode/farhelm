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
