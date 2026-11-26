## Since the last review

The landing monitor found three issues in the first delivery. It chose to keep the YOLO question available when a host
reports no installation identity and the answer can go back on the connection that refused the launch. A fresh launch
must ask again after a wake or reconnect. It also asked to report a successful Restart hook save when the launch has
changed, and to restore the exact repository suggestion alignment check.

#4999 now binds the one-time answer to the original connection when the host has no installation identity. It also pins
each edited Restart to the connection chosen at dispatch, so a reconnect the desktop has yet to see cannot redirect the
answer. A verified answer can still follow a recognized reconnection to the same installation. #5003 shows a separate
save acknowledgment and asks for a fresh press when the shown launch changes. #5037 verifies the repository suggestion
panel against the field and measures the scroll viewport separately for Recent flags. #5029 was restacked with the other
drafts. There are four draft PRs.

## What this was about

A suspended sandbox or stopped container could wake after **Start YOLO session anyway** and repeat the question. New,
Clone, Replace with and Restart with can now continue after wake or a checkout preview using the one-time answer for the
same verified installation. When an unverified host needs a wake or reconnect, the old answer stops at that boundary. On
the same live connection, the one-time answer sends the launch as usual.

When selecting Grok or Antigravity, **Set up and launch** enables the shared Restart hook setting for that agent. Future
launches use that setting on any host. A remembered folder could arrive while that save was pending, letting the launch
use a folder the confirmation had never shown. The answer now launches only if the draft was ready when the user
answered and still points to the shown destination. When the save succeeds after the launch changes, the form confirms
the save and asks for a fresh press. When the draft stays unavailable, it confirms the save and tells the user to press
the action once the draft is ready.

The launcher could report a saved Custom agent as deleted while its registration list was loading or a read had failed.
The Agent and Command tabs now keep the saved ID visible and explain the wait or failure. A retained entry from a
successful read remains usable, and only a successful list that omits the ID produces deletion advice. Restart with can
keep the required conversation field available while the registration is unread.

Keyboard users can now read the full Custom approval flags for the highlighted Recent setup before Enter. Both launcher
layouts keep those arguments below the scrolling results. Page Up and Page Down scroll long flags without moving focus
or choosing another setup.

## Things you should know

Edited Restart requests use the same connection check as Create and Replace with. Plain Restart continues to relaunch
the session's saved choice. After a visible and verified same-installation reconnect, the edited retry uses the new
connection and the original answer. A connection change that happens before the desktop can see it requires the user to
review the launch again.

The repository suggestion panel was already aligned with the input. The width test had followed the scrolling rows when
the panel's border moved into an outer element. The test now measures the painted panel, and the Recent flags test
measures the rows separately. Both keep their original geometric expectations.

## Open questions and possible follow-ups

None.

## PRs

- [#4999 Keep one-time YOLO approval through host wake](https://github.com/scode/farhelm/pull/4999/changes)
- [#5003 Tie hook setup to the destination shown](https://github.com/scode/farhelm/pull/5003/changes)
- [#5029 Distinguish missing Custom registration states](https://github.com/scode/farhelm/pull/5029/changes)
- [#5037 Show saved Custom approval flags in keyboard recents](https://github.com/scode/farhelm/pull/5037/changes)

## Checks run, reused and skipped

- The recorded `cargo nextest run --locked -p farhelm-gui -p farhelm-helm --lib -E <selection>` on `bf8f55c5` passed all
  189 tests (run `f133b639-343b-40a0-9f1b-73a2fa9b85b7`). It covered same-connection and verified-wake answers, stale
  restart routing, checkout alignment, narrow and wide Recent flags, hook setup, and folder history. The final
  hook-state amendment affects the hook acknowledgment only.
- The exact repository alignment test via recorded `cargo nextest run --locked -p farhelm-gui --lib -E <test>` (run
  `19b70985-514e-4783-9dd9-8be2be79e0b4`) showed the scrolling rows starting one pixel inside the painted border. The
  intermediate run `a52efe0e-2657-43d7-bbc3-9ece50aafebe` passed 187 of 188 cases and identified the Recent viewport
  measurement in the same bounds marker. The subsequent passing run covers both assertions.
- `cargo clippy --locked -p farhelm-gui -p farhelm-helm --lib --tests -- -D warnings` passed at `8b571d8c`. Cargo and
  Markdown formatting, changelog format, test-delay and whitespace checks passed. The instructions checker passed with a
  pre-existing archive warning.
- The final recorded `cargo nextest run --locked -p farhelm-gui --lib -E <selection>` passed 30 tests on `8b571d8c` (run
  `f5ae9e40-d1b6-4e32-a398-43e04f7b0eed`). This reran the actual folder edit during the setup write and the adjacent
  custom-registration checks after moving the save acknowledgment to its own state.
- Recorded `python3 scripts/check-gui-goldens.py --render-timeout 150 --scene <selection>` matched the published light
  and dark references for nine YOLO, wake, hook prompt, checkout and Recent scenes at `8b571d8c` (run
  `1280ecf6-8427-4664-be04-b829196f5566`). The new acknowledgment after a save is covered by the rendered GUI test.
- The delivery rebase added main's recipe-title and channel changes and the adjacent explanation of checkout preview
  waits. The stack's patches kept their behavior. The tested launcher tree and edited Restart routing still apply.
- The 16 registration and Recent references published during the first delivery still cover those rendering changes. The
  native macOS and full workspace suites were skipped in favor of the focused Linux GUI, helm routing and appearance
  checks for this round.

## Review gate

Independent GPT-6.2 Sol high source rechecks cleared the three revised PRs. The rechecks led to pinning every edited
Restart dispatch, keeping the successful hook-save status separate from other notices, and distinguishing panel and
viewport bounds in the tests. The registration layer has its original source review and an adjacent merge recheck.

### Landing

#### What landed with it

Only this plan landed this round. An earlier landing blocked it because its YOLO fix stopped the launcher from asking
the YOLO question at all on a host whose supervisor reports no installation identity; the landing monitor answered the
block on your behalf (the decision is in the plan's Decisions and a "Plan decisions to revisit" TODO entry), and an
executor reworked the four PRs. Since the rework's base, main gained only the drift and brick test tooling triage, which
touches nothing here, and the rebase applied without conflicts.

The landing's independent review found each decision implemented as answered. A host without an identity gets the YOLO
question again, and its one-time answer works when it goes straight back to the same connection: the launcher records
the connection the launch was first sent on, the request carries it, and the helm checks it before its YOLO check for
New, Clone, Replace with and, newly, Restart with. After a host wake, or a reconnect the launcher has seen, the answer
is dropped with a "Launch again" note, and the helm refuses a reconnect the launcher has not seen yet. Hosts that report
an identity keep the anchoring as first built. The launcher test that was one pixel off now passes. My block question
said the suggestions list was one pixel wider than its field; the executor found it was not: since #5037 moved the
list's border onto an outer panel, the test had been measuring the inner rows. The rework moves the marker the test
measures onto the painted panel (and trims the list's height limit by the border's two pixels), with the test itself
unchanged, so nothing users saw was misaligned. And a hook setup answer whose shown launch changed now says "Restart
setup was saved. The launch changed. Review it, then press Launch again."

#### Fixes made while landing

- None. The review found no blocker or should-fix item.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on the reworked PRs rebased onto main as it stood at the start of the landing (`d7ae4daa`), which applied without
conflicts; every file's change is exactly what the executor pushed.

- The GUI crate's tests, `cargo nextest run -p farhelm-gui`, on the test worker (recorder run
  `0ac87c29-380d-4a12-9661-b5d13c6945bb`): all 3479 passed, including the launcher test that failed in the earlier
  landing.
- The helm and protocol crates' tests, `cargo nextest run -p farhelm-helm -p farhelm-proto`, on the test worker
  (recorder run `bfc3c991-7450-4d75-8269-0e8834cebc05`): all 2243 passed. The rework added the connection check to the
  helm and an optional request field to the protocol crate; it is part of the app's own request to its embedded helm,
  not the supervisor protocol, so no protocol version changes.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `edd5d7ef-ca92-4fbf-a21d-7b082eb73bad`): of its 49 steps, 48 passed, including the launcher and its YOLO question, and
  one (paste) was skipped by design on Linux.
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `baad4f8d-7380-4091-99cf-e356d0b83876`): passed, including the plan's eight new scenes (16 pictures, light and dark);
  and the screenshot batch tool's test with the scene list (recorder run `9fec7cbc-c56d-49e3-b3e2-1e6be50b5747`),
  locally: passed.
- `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format`, `cargo fmt --all -- --check` and
  `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally: this plan's files pass; it fails
  only on over-long lines in the triage outcomes archive that main already had. No PR fixes them yet; a formatting-only
  one would, and I recommend it, as earlier rounds' notes did.

Reused: none. The executor's runs were on an older base, and everything they covered ran again above.

Skipped: the rest of the workspace Rust suite, since only the GUI, helm and protocol crates changed and their tests ran
in full; the desktop app's outer shell tests, since nothing in it changed; a Mac run; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing in the report. My own block question misdescribed the one-pixel test failure, as the first section says.

#### Things you should know that the report does not say

- **Smaller points the review noticed.** **Launch without** on a hook setup question whose draft can no longer launch
  now clears the question and shows nothing (no setting is written, so it is harmless); a YOLO refusal of a new
  disposable environment would show the "Launch again" note in a loop, which is probably unreachable since new
  environment hosts start with YOLO allowed; SPEC_impl.md still says no new precondition checks are added although
  Restart with now carries one, which the same section states a few lines later (a one-line docs fix the next
  SPEC_impl.md edit there can make); a few branches have no test of their own (the guard that stops a wake for an
  identity-less answer, the same-connection answer for anything but New, and the hook note's "Press Launch when the
  draft is ready" wording); and the YOLO and hook changelog drafts could use a context paragraph for the curator. None
  needs a follow-up on its own.
