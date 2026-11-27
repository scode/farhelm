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
