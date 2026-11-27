# Report: the release brick test no longer looks for the removed Welcome card

## What this was about

The desktop app used to open a Welcome card over the session list on first start, dismissed with **Got it**. #5012 and
#5018 replaced it with a Welcome view inline in the empty main pane, with nothing to dismiss. The macOS release brick
test was never updated.

The brick test hands the tester inside the Mac guest each check's instructions in one of two forms. Live runs always use
a short form that names the harness's fixed operations. A longer, free-form one also exists in the harness, but a live
run does not send it. In the short form, the candidate's first open after a fresh install still said, without a
condition, to dismiss the Welcome card with **Got it**, so the next brick test would have sent the tester after a button
that is not there. The candidate's relaunch after Restart to update already said "if present". The longer form was
conditional on every start, and the runbook said to dismiss the card on first open. Nothing has gone wrong yet, because
no brick test has run on a build without the card. The TODO entry asked for this fix.

When the plan was written you decided to drop the dismissal from both candidate starts (fresh install and the relaunch
after an upgrade), and to keep it, made conditional, only for the previous release that an upgrade test installs first.

What is on main now:

- The candidate's fresh-install start and its relaunch after an upgrade no longer mention a Welcome card at all, on any
  update path, in either form.
- The previous release's first open dismisses the Welcome card with **Got it** only if it appears, in both forms (the
  short form used to say it unconditionally there too). The baseline can be any of the three newest stable releases, so
  it may predate the card, show it, or postdate it, and the wording covers all three.
- The longer form's line telling the tester not to press Send feedback or start the feedback test early stays on all
  three starts. The short form never had that line on the candidate's starts; see the follow-ups. The inline Welcome
  still offers Send feedback, and the live feedback send belongs to its own later check.
- The Announcements dialog's own **Got it** dismissal is unchanged.
- The runbook's startup paragraph now says the candidate shows Welcome inline with no Welcome card to dismiss, and that
  a previous-release baseline may still show the card, to dismiss if it appears.
- Two unit tests pin the instruction text above; the runbook paragraph has no test. The TODO entry is removed.

## Things you should know

- **This is not verified against a live brick test.** The tests check the instructions the tester receives, not what the
  app shows inside the Mac guest. The next real brick test is the first live use.
- **The TODO entry had suggested keeping the conditional dismissal on the relaunch after an upgrade.** I followed your
  decision in the plan instead, and dropped it there too. The app that comes back after Restart to update is always the
  candidate, so it has no card.
- **No changelog entry.** This is a `test` change to the release test harness, with nothing a Farhelm user can notice.

## Open questions and possible follow-ups

- **The candidate can still open other startup dialogs that close with Got it, and the tester is not told about them.**
  The reviewer pointed out two. Announcements opens on the relaunch after an upgrade when the release selected in-app
  announcements. "What's new in Farhelm" can open on a stable candidate once the website lists it, which is uncommon,
  since the website is usually deployed after the brick test. This change did not make that worse: the old instructions
  never mentioned either dialog either. I only narrowed the runbook's wording to "no Welcome card", so it does not claim
  that nothing opens at startup. If you want the tester told to dismiss them when they appear, that would be a small
  follow-up.
- **Real brick tests give the tester no "do not press Send feedback" line on the candidate's starts.** The short form,
  which live runs use, limits feedback only on the previous release's first open ("no session or feedback yet"). The
  longer form carries the guard on every start, but a live run does not send it. This was already the case before this
  change. The inline Welcome's Send feedback link makes an accidental early send a little more reachable than the old
  card did. Adding the same sentence to the short form's two candidate starts would close it.

## PRs

- #5132 test: stop brick tests dismissing a Welcome card on the candidate (the tester's instructions, the runbook
  paragraph, two unit tests, the TODO removal).

## Checks

Run now, on the change as it lands, through the test recorder:

- Run `7441d487`, `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v`: the whole portable
  brick-test suite, 337 passed, none skipped. That includes the two new tests. The first checks that neither candidate
  start mentions a Welcome card or **Got it** in either instruction form, on every pass and update path. It also checks
  that the previous release's first open has exactly one dismissal, worded as conditional, and no other mention of **Got
  it**. The second checks that the longer form keeps the don't-send-feedback line on all three starts.
- `dprint check` on the runbook and TODO.md, clean. `python3 scripts/check-agents-md.py`, no errors. Its one warning is
  about a citation in the frozen triage archive and was already there.

Reused: none. Superseded: run `f394ec04` (the same suite, 337 passed) ran before the review fixes.

Skipped: Rust, desktop and end-to-end tests, since nothing outside the brick test's instructions, runbook and their unit
tests changed. A live brick test was also skipped: it needs the Mac test machine and a release candidate, and the
procedure runs it at release time.

## Review gate

A fresh-context Opus 5.5 review at high effort found no blocker. It confirmed that every acceptance point in the plan
holds, and that nothing else in the brick-test files still tells the tester to dismiss a startup Welcome card. It also
confirmed that the card first shipped in v0.29.0, so older baselines really do predate it. Its one should-fix was that
my runbook wording ("no startup card to dismiss") overclaimed, because of the two dialogs under follow-ups; it now says
"no Welcome card". Its two nits were also fixed:

- a test docstring overstated what the short form says about feedback;
- one assertion was case-sensitive where it should not be.

The fixes were not reviewed again. Run `7441d487` ran after them.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained the previous rounds' changes to the app, the helm, the supervisor
and the command line. None of them touches the release brick test's instructions or the app's startup screens. In the
same round, three more plans landed: remove-legacy-sandbox-sessions-api (an unused helm route removed),
restart-startup-scope-cleanup (a supervisor start finishes a failed Restart's cleanup) and claude-drift-trust-copy-back
(a drift test). None touches the brick test.

An independent reviewer and I read the change and found nothing to fix.

#### Fixes made while landing

- **A TODO.md conflict.** The round's four plans each removed their own TODO entry, next to each other, so stacking them
  conflicted there. I removed all four entries; no code changed.

#### What the landing made untrue in the report

Nothing I found.

#### Worth knowing

- **The follow-up about other startup dialogs has to word around a new test.** The report suggests telling the tester to
  dismiss Announcements or "What's new in Farhelm" if they appear on the candidate. Both close with **Got it**, and this
  plan's new test forbids the words "Got it" in the candidate's start instructions, so such a follow-up has to name the
  button another way or change that test.

#### Checks

Run once, on the round's four plans stacked on main as it stood when the landing started
(remove-legacy-sandbox-sessions-api, restart-startup-scope-cleanup, brick-test-inline-welcome, then
claude-drift-trust-copy-back), so each plan was tested together with the others and with the previous rounds' changes,
which none of the executors' own runs included. The SSH test worker was still unreachable, so the slow suites ran on
boat sandboxes (x86_64 Ubuntu 24.04 VMs), and clippy ran locally:

- Locally, `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`: clean.
- On three boats, every test of the protocol, helm, supervisor and command-line crates that runs by default, the
  end-to-end suite included (`cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor -p farhelm`,
  built once, split in three and run through the recorder), recorded runs 0aeec1d1-374d-4651-b817-c6196eba36e3,
  79bc70f6-81f9-4677-98ac-5bc0782c1540 and 0e083ecd-f092-4996-ae93-b1866a1bddfd: 4,732 passed, none failed, and no test
  that quietly passed for lack of systemd, Podman or SSH on the machine. That covers each executor's own selection and
  more: the helm's Tensorlake launch tests, the Restart cleanup tests, the startup tests of terminal hosts left over
  from earlier runs, and the drift tests' private-configuration tests.
- Locally, the portable brick-test harness
  (`python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v`), recorded run
  69a626cf-42a8-4fcc-8baa-85e5c53b0de6: 337 passed.
- Locally, `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`),
  `python3 releasing/check-changelog.py format`, `dprint check` on the changed Markdown, and
  `python3 scripts/check-agents-md.py` (the brick-test runbook is an instruction file): passed.

Reused: nothing. Skipped: the desktop app's and GUI crate's tests, the screenshot comparison and the end-to-end smoke,
since the only change to the app is two comments; the opt-in tests in those crates, among them the real-agent drift
tests, which spend real Claude turns; and a live brick test, which needs the Mac test machine and a release candidate.
