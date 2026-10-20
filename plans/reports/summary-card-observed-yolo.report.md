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

### Landing

Landed as #2378 in its own landing round, unchanged from what the report describes.

#### What landed since the stack was based, and whether it interacted

Since the PR's base, main gained only plan bookkeeping, and while the landing ran, two documentation changes to the
review queue's triage instructions and records. None of them touches the UI. The recent UI changes that this plan sits
next to, session-header-and-row-fixes (session questions, copy warnings and accessible names in the session list) and
workspace-trust-destination (a trust note on approval cards), were already in its base; a fresh-context reviewer
confirmed this diff touches none of the code they changed. host-entry-session-ids, still in progress, will change how
sessions are identified, but this plan does not touch session identity, so at most a small textual conflict in the same
SPEC.md paragraph is possible when that plan lands. The landed code is identical to what the landing's checks below
tested.

The reviewer also confirmed that one shared check now decides when the row's shield shows YOLO, the shield's hover and
screen-reader text, and the card's new suffix, so the three cannot disagree; that launches with no known agent type, and
agents Farhelm cannot read a status line for, cannot gain the observation; and that the actions-menu header still
describes only the stored launch.

#### Fixes made during the landing

None.

#### Possible follow-ups

- When a session has an explicit model or effort, the suffix comes at the end of the line, so a guarded Claude session
  reads `claude · guarded · model: opus · effort: High, YOLO observed`, where the observation can look as if it is about
  the effort. Putting it straight after the permission (`claude · guarded, YOLO observed · model: …`) would be a small
  change. The plan's own test covers only the case with no model or effort.
- The test checks that a session with no launch record gains no observation, but not that an agent Farhelm cannot read a
  status line for (Codex, say) gains none when the YOLO evidence flag is set. The code handles it correctly by reading;
  one more test case would pin it.

#### Checks

Run now, on the PR rebased onto main:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, `dprint check` on the changed Markdown, and
  `python3 releasing/check-changelog.py format`: all passed.
- Through the test recorder: the whole UI library, 549 passed, run `3b25dbea-830d-41c5-90ce-eda4cd37e882`; the same UI
  library built with desktop support, 707 passed, run `079a97d7-69ff-4c76-9bbf-7622a95643f1`; `node --test` in the UI's
  JavaScript test directory, 311 passed, run `ef285442-edc4-4147-82b4-75ea5f9b1d44`.
- After `cargo build` and the release web build,
  `npx playwright test tooltip.spec.ts sidebar.spec.ts yolo-guard.spec.ts guarded-permissions.spec.ts custom-permissions.spec.ts`
  through the recorder on Chromium and WebKit, one worker, no retries: 286 passed, run
  `73a3cadc-6111-4dd8-acd9-3b16ca46763a`. These render the hover card, the session rows and the permission shields.
- Skipped: the helm, supervisor and other Rust libraries (this plan changes only the UI crate), the rest of the browser
  suite, and the native Mac app.

Nothing in the executor's report became untrue at landing.
