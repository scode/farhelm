## What this was about

A session's notification list could lose keyboard focus when a layout change closed it. It could also remain beside the
wrong row after deleting a session above it. Triage approved fixing each within a 100-line code, test and documentation
limit, discarding either outcome if it exceeded that limit.

Closing the list now returns its own focus to the bell while preserving focus the user moved to another control. Local
deletion closes a list whose row moved, using the same displayed-order rule as the actions menu. Both outcomes are
complete, at 59 and 83 changed lines respectively, excluding ledger, queue removal and changelog fragments.

## Things you should know

The changes are two draft PRs, ready for the separate landing monitor. Each includes its feedback cleanup, execution
record and changelog fragment. No scope or size fallback was needed.

Browser development exposed two test setup problems. Changing window width did not resize the fixed-width sidebar; the
focus test now changes height and verifies that the sidebar resized. Opening a bell while another row's actions menu was
closing exposed the separate focus race described below. The deletion test now finishes that preceding menu handoff
before opening the bell, while keeping deletion pending. The original failures remain retained; later passes do not
erase them.

Main advanced only through queue claims and another plan's blocked question. Those diffs were inspected; they do not
interact with these changes, so no rebase or additional runtime validation was needed.

## Open questions and possible follow-ups

Switching directly from one row's actions menu to another row's notification list can let the closing menu take focus
back and immediately dismiss the new list. A retained Chromium trace demonstrated that sequence. This plan isolates it
from deletion and leaves it for a separate follow-up; its fixes concern automatic notification dismissal and movement
after deletion. No maintainer decision is required to land these two outcomes.

## The PRs

- [#2427](https://github.com/scode/farhelm/pull/2427/changes): return focus to the bell when automatic dismissal removes
  the focused notification list; head `b833bb78dcc57c957271e470b2c7adb327d1574f`.
- [#2430](https://github.com/scode/farhelm/pull/2430/changes): close a notification list when local deletion moves its
  row; head `628a8c077389bea04baf24bfa47c43fbe5a3b103`.

## Checks run, reused and skipped

Run for this plan: `cargo build`; Dioxus 0.7.10 release web builds for each product change;
`cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`; changed Markdown `dprint check`; the isolated
test-sleep checker (305 delays, zero missing rationales); and `python3 releasing/check-changelog.py format`. All
applicable checks passed. A TypeScript-only dprint invocation reported no matching files because this repository's
dprint configuration does not format TypeScript; the changed Markdown check passed separately.

Runtime checks used the recorder, nextest 0.9.148 or Playwright 1.64.0, pinned tmux 3.7c, zero retries, four nextest
slots or one browser worker. Browser selections ran Chromium and WebKit. Records have complete source and reports,
retained output through EOF, and complete cleanup:

| Run ID                                 | Selection and result                                                                                                                                    |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `88a9f16a-932d-4d23-91da-a9814966d075` | `cargo nextest run -p farhelm-ui --lib -E 'test(list::bell::)'`: 1 passed, 554 filtered out.                                                            |
| `11a917ba-0783-4aa1-80af-4eaa7bba751b` | `cargo nextest run -p farhelm-ui --lib -E 'test(rows::tests::an_optimistic_removal) \| test(rows::tests::a_held_removal)'`: 2 passed, 553 filtered out. |
| `6ddc98d9-fe15-4344-89cc-a790a1a1fa71` | `npx playwright test 'notifications\.spec\.ts'`: 16 passed, 2 failed before the focus assertion because width did not resize the sidebar.               |
| `d915d18b-e5b4-4777-9b9d-50a922457bd5` | `npx playwright test 'notifications\.spec\.ts' -g 'automatic notification dismissal returns'`: 2 passed after correcting the resize premise.            |
| `4e14efc7-9a56-43a1-adf9-2df1d360f6b6` | `npx playwright test 'notifications\.spec\.ts'`: 19 passed, 1 failed during the preceding menu-to-bell handoff.                                         |
| `a790e16f-e9e0-4446-b96c-2f294da9a5a7` | `npx playwright test 'notifications\.spec\.ts' -g 'a local delete closes the notification list'`: 2 passed after settling that handoff.                 |

The backend build covered `6e83988f`'s backend source; outcome 1 changed only UI code and its tests. Outcome 1's web,
Clippy and unit checks covered `6e83988f` plus its bell change, unchanged in final head `b833bb78`. The unit source
fingerprint is `383dfb0a3c7bbb747dc35be9a5ef4f9c7b81db05347e163e35ea8d4904354526`. Outcome 2's web, Clippy, unit and
full-browser checks covered `b833bb78` plus its movement fix and original browser fixture; both recorded runs have
fingerprint `8ebe7cf759d4cc084803054a7495dd4bcc1cf148d86e5efcf3075b835a6a44f8`. The final narrow browser run captured
base `b833bb78` plus the movement fix and reviewed fixture correction, fingerprint
`192735c094bada6d425dc02f681de4850311da3fd3382de753a9daa9bce69e7c`. The second PR's subsequent URL amendment changes
only the execution ledger. Its two corrected cases, together with the 18 unchanged notification cases that passed in the
preceding run, cover the final 20-case selection. This is reuse of applicable passing cases, not a claim that either
failed full invocation passed. The second outcome's unit, Clippy and web-build evidence remains applicable because its
only later source edit settles the browser fixture. The first bell unit's Rust source also remained unchanged through
its later browser correction. The backend build was reused for outcome 2 because no backend source changed. No runtime
substrate skip was observed in retained output.

Skipped broader Rust, browser, desktop, installer and CI runs: the selected unit and browser checks cover these two
notification interactions, and no remaining coverage risk requires those suites. CI is manual-only. No latent-flake
entry was added for failures encountered and corrected within this session.

## The review gate's outcome

Each PR passed exactly one fresh-context gpt-6.1-sol/high review, with corrections checked by that same reviewer. Review
prompted terminal-readiness coverage in the first test; runtime evidence prompted the resize and preceding-menu boundary
corrections. There are no outstanding findings in either final diff. A fresh scope review found no unnecessary product
mechanism, and its readiness-boundary conclusion also applies to the final fixture correction. The resume check and both
commit-message cold reads passed. The report received its separate cold read before delivery.
