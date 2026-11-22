## What this was about

The November 21 review found duplicate logic behind approval cards, the Hosts panel, Settings, and the session list and
pane. The existing copies worked, but a future fix could make one approval card let a user click while a reply was
pending and leave another card blocking the same action. The maintainer divided the work among four plans and asked that
the requests, refusals, keyboard behavior, and screenshots match the existing app.

## Things you should know

- Host-add approvals and tunnel warnings share the checks for a waiting approval or an answer in progress. A tunnel
  warning stays tied to its original approval across a reconnect. Commands that do not tie up a particular row or form
  share how they interpret the helm's answer, while each caller can still handle a connection error.
- Remove and Delete both close the host actions menu before proceeding. Suspend and Resume share their verb handling, so
  the provider line can still say “start” when the environment is starting. Progress, status, and uninstall continue to
  refresh independently. Saving a permanent answer clears its prompt when the save starts and presents any refusal in
  the words of the panel that asked.
- Changes to one preference still send only the fields that changed. Closing a Settings editor returns focus to the
  target chosen by that screen, which can be Add or New after an edit. Execution found the same focus wiring in the
  recipe editor embedded in Add Host and reused it there.
- Stop, Restart, Delete, and related session actions share how they start requests and detect work already in progress.
  Each action keeps its own confirmation, messages, and progress timing. A move of several marked sessions still sends
  the moves in order and keeps the affected rows busy until their turn.
- Pending Stop and environment actions put a spinner before the title. Pending Delete fills the title slot. A stale
  compact row keeps its clock, and a full row keeps the cause text. Both give the same explanation on hover and to a
  screen reader. The Details bar and unavailable band share status and time-since-activity calculations and keep their
  existing visibility rules.
- Session list tests share frame reads, clicks and scrolls, and acknowledgments from the fake helm. Their checks still
  distinguish answers in separate rows and replies in send order. The final PR removes this plan's seven TODO items. The
  remaining launcher and terminal-control work stays on the shared entry, and the wide-glyph plan's removals carry
  through the rebase.

## Open questions and possible follow-ups

None.

## Pull requests

- [#4597](https://github.com/scode/farhelm/pull/4597/changes) — Approval lookups and helm reply classification.
- [#4601](https://github.com/scode/farhelm/pull/4601/changes) — Hosts panel action and state helpers.
- [#4605](https://github.com/scode/farhelm/pull/4605/changes) — Sparse preference commands and editor focus restoration.
- [#4610](https://github.com/scode/farhelm/pull/4610/changes) — Session request admission and pending work.
- [#4612](https://github.com/scode/farhelm/pull/4612/changes) — Session row progress and stale status rendering.
- [#4614](https://github.com/scode/farhelm/pull/4614/changes) — Session pane status and activity age.
- [#4619](https://github.com/scode/farhelm/pull/4619/changes) — Session list test helpers and the plan's TODO cleanup.

## Checks run, reused and skipped

- **Final stack.**
  `scripts/record-test-run.py --runner nextest -- cargo nextest run -p farhelm-gui --lib -E <focused selection>` passed
  222 cases at `1e60ce4e` (the same tree as `317a297c`). It covers marked rows staying selected when a Shift-drag
  crosses a collapsed group, shared frame helpers, and pane lifecycle and terminal behavior after rebasing onto the
  wide-glyph protocol change. Recorder `a11c1a07-e694-4b4c-9230-bbcc80be22c6`.
- **Screenshots.** `scripts/record-test-run.py -- python3 scripts/check-gui-goldens.py --jobs 4 -j 3` matched all 406
  Linux references on that final stack. All drawing PRs also passed full compare-only runs at their review checkpoints.
  The final run uses main's launcher and wide-glyph references. Recorder `7bbc5033-9f5c-45c8-82e2-9fed9996f3c1`.
- **Reused focused tests.** The range-diffs confirmed the production patches stayed identical over the rebases. The
  final pane and screenshot runs cover the relevant interaction with main's terminal changes. Approval and reply cases
  passed 124 at `f846fa93` (recorder `e2483b55-2c97-4f3f-abf0-f5184dba922f`). Hosts and Sidebar passed 293 at `28a50469`
  (`8cbaef4c-e43e-476c-9d4c-6024f262d360`). Settings and callers passed 358 at `4cda4c70`
  (`18b3877d-1f09-48ef-a913-13f9d5f63940`). Session action tests passed 415 at `ad861a13`
  (`bd97a058-08d3-4b98-8871-b23015488f70`), with 29 focused cases after the final Suspend lookup at `d50da7e9`
  (`667691b5-0f3b-46c6-afc2-813998a1886d`). Row tests passed 179 at `51e91b5f` (`df321297-e8ab-423b-8244-1fc697f3eca3`),
  and pane status and age tests passed 62 at `6e9ca2c8` (`fe454835-d5f6-4f4f-a7b3-97438f394407`). The broader
  test-helper selection passed 906 at `c79c35c8` (`65667868-6c44-4dfa-a1f8-e58ea1959556`). The final run covers the
  newly shared acknowledgment in the Shift-drag test and the affected rebased paths.
- **Static checks.** Rustfmt and `git diff --check` passed on the touched files, and
  `python -B scripts/check-test-sleeps.py` passed with the pinned isolated parser on the final test-helper patch. The
  first Hosts validation stopped at a Rust borrow error (recorder `e44d0eb8-8850-40c5-8077-3e5e58acae47`). The corrected
  retry is the 293-case Hosts and Sidebar run above.
- **Skipped.** The workspace-wide test battery, release and installer suites, real providers, and separate visual review
  were unnecessary for these GUI refactors. The focused tests exercise requests, refusals, drawing and focus. The image
  comparison verifies the rendered references.

## Review gate

Each PR passed an independent fresh-context source review. Findings corrected an unused approval accessor and an
inaccurate comment, a borrowed Hosts reply, preference route ownership and focus documentation, the remaining Suspend
lookup, the pane's badge documentation, and the newer test drain and its helper documentation. The row rendering review
found no issues. The final test-helper review verified both plans' TODO removals. Commit wording passed separate cold
reads.

### Landing

#### What landed with it

The plan's seven PRs (#4597, #4601, #4605, #4610, #4612, #4614, #4619) landed first in this round, followed by the
launcher and templates helpers and the host command status change. Since the plan was written, main had gained the
terminal and controls helpers (landed in the previous round), one of the four plans that work through TODO.md's
"Consolidate the helpers duplicated across Farhelm" entry. This plan uses some of what that one made shared but changes
none of the same code, so the only conflict was in that TODO entry, which the landing settled by applying this plan's
own edits to main's version.

An independent reviewer read the plan line by line against its plan file and found nothing to fix: the errors and
warnings the hosts panel and session list show, their order, and the look of a session row's progress title are
unchanged.

#### Fixes made while landing

None.

#### Checks

The round landed two refactors of the desktop window's code and a change to how the helm learns whether a host setup
command succeeded, so the landing ran the whole Rust workspace, the desktop app's tests, the lint runs, the end-to-end
smoke test and every reference screenshot, once, on the three plans stacked together. The worker runs were on a separate
arm64 Linux test worker with a systemd user manager and SSH to itself; the rest ran locally.

On the three plans as delivered, rebased onto main and stacked:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `068b2cdd-019d-458c-8d42-853d8baa039b`): all 8342 passed, and none printed a runtime `SKIPPED`, so the tests that need
  the systemd user manager, Podman or SSH ran rather than returning early. The 86 tests marked ignored (benchmarks and
  opt-in real-host and real-Mac tests among them) did not run, as on every run.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and both desktop app
  clippy runs (with and without `--features agent`), on the test worker: passed. Lints are not runtime tests, so they
  have no recorder run.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `d7399edb-dd5b-4c27-bdff-c8afce3a87d9`): all
  185 passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `dbfad242-d312-4bbc-8951-85c55a6a19f7`): 49 of
  its 50 steps passed and the paste step was skipped, as on every Linux run.
- `python3 scripts/check-gui-goldens.py`, every scene, locally (recorder run `87d4fc87-f934-4921-977d-06508c7234bb`):
  every reference screenshot matched.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and `python3 -B scripts/test-gui-screenshot-batch.py` (the screenshot
  comparison page's own tests), locally: passed.

After the changelog fix on the host command status plan (#4629), which touches only a release-notes draft:
`python3 releasing/check-changelog.py format` and `dprint check`, locally: passed. Everything above still applies.

Skipped: macOS, since nothing in the round builds only on a Mac; for the host command status plan, a real Tailscale SSH
Mac and the real Tensorlake gateway, which its report names as post-landing checks; CI, which runs only on demand.

#### Things you should know that the report does not say

- Moving several marked sessions into a group still sends each move its own way rather than through the shared helper
  the single-session move now uses. That is correct (the batch has to stay tied to the list view it was started from),
  and the report says so, but the plan also asked for each such deliberate exception to be listed, and the report only
  describes this one in passing. Nothing to do unless you want the report's list completed.
- A debug-level log line written when a closed session list refuses a request is gone. Nothing visible changes, and
  nobody is likely to miss it.
