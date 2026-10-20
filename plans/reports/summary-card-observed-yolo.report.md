## What this was about

A session's hover summary could say `claude · guarded` after Farhelm had seen the agent's status line report that it
bypasses permission checks. The row's shield already showed YOLO, but the card did not explain the disagreement. The
maintainer chose to retain the stored launch setting and mention the observation in the card; the actions-menu header
continues to describe the stored launch alone.

The card now adds `YOLO observed` when the shield has supported status-line evidence. The card, shield and shield's
description share the evidence check. The regression renders the actual card and checks its wording alongside the
unchanged menu summary.

## Things you should know

An already-YOLO launch also gets the observation suffix: the chosen launch setting and what Farhelm saw on screen are
separate facts. Command launches with a declared supported agent get it too, matching the shield. Unsupported and legacy
launches cannot acquire an observation just from the evidence flag. No guard, permission inheritance or approval rule
changes; the observation remains advisory and best effort.

The change is 99 lines of code, tests and specification diff, below the agreed 150-line gate. The ledger records that
the size gate replaced the earlier time estimate. SPEC.md describes the card/menu distinction; SPEC_impl.md describes
the card's rendering mechanics but does not specify its agent-line permission wording, so it needed no change.

## Open questions and possible follow-ups

None required for this plan.

## The PRs

- [Draft #2378](https://github.com/scode/farhelm/pull/2378/changes): show observed YOLO in session summaries.

## Checks run, reused and skipped

Run during execution:

- `cargo nextest run -p farhelm-ui --lib -E 'test(list::summary_card::) | test(observed_yolo_escalates_the_drawn_shield_and_keeps_launch_context) | test(menu_header_uses_saved_launch_facts_and_existing_state_words)'`
  through the recorder with pinned nextest and tmux, four global slots and no retries: 5 passed, 544 selected out, no
  failures or flaky outcomes. Recorder run `66d3ba0d-e9ba-419f-b9b2-5c9769a9022d`, tested base
  `21e198ea79bf2efc459d11e1d8915dfdf9a15f48` plus the working diff, source fingerprint
  `40c6f88111013858607478f340d7fc25a89e19d52d1cefe5f2f11065264c746e`. The complete retained output had no runtime
  `SKIPPED` messages; the selected tests ran, rather than returning early for unavailable infrastructure.
- `cargo clippy -p farhelm-ui --lib --tests -- -D warnings`, `cargo fmt --all -- --check`, `dprint check` on the changed
  Markdown, `python3 releasing/check-changelog.py format`, and `python3 scripts/check-agents-md.py`: passed.
- The isolated parser's `python -B scripts/check-test-sleeps.py`: passed, 304 delays inspected, none missing a reason.

These successful checks still cover the final change after its careful rebase onto
`7e4772f50fad2349f33701545ddd25c7ff274e08`: the full intervening diff contains the separate supervisor trash-location
fix, its landing report and plan bookkeeping. None changes the UI summary, its evidence predicate or callers. The
summary's code and test diff stayed identical, and the later ledger URL/size update changes no executable behavior.

Skipped the full workspace battery, desktop runtime, JavaScript and browser tests: this change adds text runs to an
existing card source, covered by real Dioxus rendering and the shield/menu regressions. It changes no tooltip binding,
layout, JavaScript or desktop lifecycle code. No older executor evidence was reused.

## The review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no issues. It checked the acceptance
criteria, shared evidence condition, rendering regression, feedback removal, specification wording and the 99-line size.
The commit title and body passed a separate fresh cold read.
