# Remember the feedback contact

## What this was about

Send feedback used to start with an empty contact field every time. The dialog now offers “Re-use for future feedback”,
checked by default while a contact is entered, so repeat feedback can use the same contact in the desktop app and web
UI.

## Things you should know

The helm remembers exactly the contact sent, including surrounding spaces, after a successful send with reuse checked.
Unchecking it or emptying a prefilled field forgets the remembered contact after success. Failed sends and Cancel leave
it unchanged. The checkbox has hover text.

The contact lives in the helm’s shared preferences and can be read by authenticated clients. Each app or browser tab
reads those preferences after authentication; another open app or tab sees changes after a reload. Every successful send
writes the remembered choice, including an apparently unchanged one, so a stale client can deliberately replace another
client’s later choice.

The feedback submission and forwarding route are unchanged. Remembering or forgetting uses a separate best-effort
preference write after sending succeeds. A failed preference write logs an error without contact details and can cost
the next reload its convenience; it does not turn delivered feedback into a failure. The existing write queue retains
unacknowledged choices for sign-in recovery, including explicit clears.

The helm database advances from schema 42 to 43 through an additive migration. Older helms cannot open the upgraded
database. This downgrade limitation was accepted during planning. Specs and the Send feedback documentation page
describe the behavior, and the TODO entry is removed.

## Open questions and possible follow-ups

None required for this plan.

## PRs

[#1689 — remember the feedback contact](https://github.com/scode/farhelm/pull/1689/changes), one draft PR based on main.

## Checks run, reused and skipped

Run on the final combined revision:

- `cargo fmt --all -- --check`, `dprint check` on the changed Markdown, and
  `python3 releasing/check-changelog.py format`: passed, to check Rust layout, docs formatting and the new entry.
- `python -B scripts/check-test-sleeps.py` with its isolated pinned parser environment: 269 delays inspected, zero
  without a rationale. This inspects the changed tests and merged template tests; it executes no tests.
- `cargo build` and `dx build --package farhelm-ui --platform web --release`: passed after the rebase, providing
  matching CLI and UI artifacts for browser validation.
- Recorder browser run `89af11ef-548f-4f0d-890f-59b79bba06e7`,
  `npx playwright test 'feedback\.spec\.ts' -g 'successful feedback clears contact when reuse is unchecked or the field is emptied'`:
  2 passed, one per engine, with complete reports. The narrow selection checks the corrected clear-contact assertion on
  Chromium and WebKit, one worker and zero retries.

Reused browser evidence from the combined revision based on `4a548d88`: run `092efd09-a737-4273-9d7f-bf680357e60e`,
`npx playwright test 'feedback\.spec\.ts' 'tooltip-coverage\.spec\.ts'`, passed 22 cases and failed the clear-contact
case on both engines. The failure was in the new test: an empty contact is sent as JSON `null`, but the assertion
expected an omitted field. The assertion, its type and its explanation were corrected without changing product code. The
other 22 results still apply, including remembering/reload, stale-tab overwrites, failed-send/Cancel preservation and
both tooltip tests. A first narrow attempt, run `ce1c5c63-0957-4a30-9581-85714d36d153`, matched no tests because its
grep incorrectly anchored the test name; it provides no runtime evidence. Both failures remain retained privately.

Reused Rust evidence from the working tree based on `b0d298b8`:

- Recorder run `374222f7-a372-453b-ad1b-ce36f40d17fc`,
  `cargo nextest run -p farhelm-helm -p farhelm-ui --lib -E 'test(preferences::) | test(store::tests::schema_) | test(preference_row) | test(feedback::tests::) | test(preference_write_tests::)'`:
  35 passed. Covers contact validation, clearing, durable preferences, successful-send choice and queued writes
  including recovery.
- Recorder run `30b60577-8542-46e3-8444-00aa4ea95aea`, the focused helm migration/fresh-schema parity selection: 12
  passed. Recorder run `9dcfed51-fed8-4cd4-b951-eb77afb84406`, the corrected historical fixtures and contact migration
  selection: 4 passed. These cover additive upgrades and existing refusal/locking contracts. All three ran with pinned
  nextest and tmux, four slots and zero retries; retained output contained no runtime substrate skips. Selected-out
  cases are not claimed as coverage.
- `cargo check -p farhelm-ui --features desktop` and
  `cargo clippy -p farhelm-helm -p farhelm-ui --all-targets -- -D warnings`: passed for the shared dialog and changed
  Rust packages.
- Frozen website dependency install and `bun run build`: passed, 31 pages and valid internal links. A later
  plain-wording correction altered no frontmatter, link or generated structure.

The careful rebase read all main changes: the template launcher-kind inference, template editor, related docs and queue
bookkeeping. They change independent spec sections and UI controls; the shared tooltip test preserves both dialog
sections. No feedback, migration or preference-queue contract changed, so those Rust results still apply. The browser
run uses rebuilt combined artifacts to cover the shared test interaction.

Skipped the full Rust and browser suites, desktop runtime smoke, installer and release checks: the focused
persistence/queue tests, desktop compilation and both browser engines cover the affected contracts. This change adds no
installer, release automation or desktop-only behavior. No hosted CI or website deployment was requested. An initial
compile-only test attempt, run `8f8cb9ef-c02a-4564-abfe-222bb2b0efc1`, failed on a missing test import and preference
fixture field; both were fixed, and its private evidence remains retained.

## Review gate outcome

Independent Opus 5.5 and gpt-6-astra reviews at high effort found no correctness defects. Opus identified seven
low-severity improvements, all applied: older-schema fixtures, a one-time preference read without a signal subscription,
SQL formatting, type/module documentation, browser cleanup, and user-facing docs wording. Astra also checked the
corrections, including the final browser null assertion against the unchanged feedback protocol, and reported no
actionable findings. The executor inspected the changes and performed the validation above. Only required reviews and
cold reads were delegated; implementation and checks were performed by the executor.

### Landing

Landed on 2026-10-06 (UTC) as #1689 (the feedback dialog can remember its contact), one squash commit on main.

#### What else was on main

Nothing that could interact: between the commit the change was built on and the landing, main gained only the planning
queue's own bookkeeping. This change takes the helm database to version 43; no other open change touches the helm's
database (the notification change in progress alongside it changes the supervisor's own database, not the helm's).

#### Review before merging

A separate reviewer that had not worked on the plan checked the change before it merged and found nothing that breaks.
It confirmed:

- Nothing else that reads or writes the helm's shared preferences drops or overwrites the new contact: every write
  changes only the fields it names, an older app build ignores the field, the replay of unsaved choices after signing in
  again includes it, and none of the screenshot, README image, demo video or desktop smoke staging writes it. The
  browser tests reset the shared preferences around every feedback test, so a remembered contact cannot leak into other
  tests.
- The contact does not reach logs, supervisors, remote hosts or agents: the helm's refusal of an over-long contact does
  not repeat it, the feedback route and the app's request logging record no contact, the full preferences are returned
  only to a signed-in app, and nothing on the supervisor, agent or command-line side reads preferences.

#### Checks

- Reused: the report's checks, including its browser run on the final revision. The code on main after the merge is
  identical to that revision, and nothing but the planning queue's bookkeeping reached main in between.
- Skipped: running anything again during the landing, for the same reason.

Nothing in the report above was made untrue by the landing.
