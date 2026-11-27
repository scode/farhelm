# Report: Settings waits for the helm instead of working around it

## What this was about

Settings had collected machinery so a person could move on before the helm answered a save. The close question watched
saves still on their way and reopened itself when they finished. The close question's Save could queue up behind a page
Save. A note at the foot of the page asked people to close again once a Test finished. The credential editor let you go
Back during a save and then lost that save's list of hosts it could not reach. The recipe editor asked its leave
question over a save already on its way. Create SSH key, which makes the key Tensorlake hosts use, checked for it on a
half-minute timer. Seven TODO entries asked for more fixes around this machinery:

- show a failed hook setup save after Settings closes;
- wait for credential saves after Back or Cancel;
- reveal the credential editor when the close question's Save fails;
- name each custom agent draft when closing Settings;
- keep credential delivery reports visible when Settings closes;
- bind the page's Save and Discard to the page and drafts shown;
- stop the close question's Save when an independent save fails.

You decided that for Settings the helm is a presumed low-latency, ordinarily reliable remote, and that making someone
wait for its answer is fine. The stack records that rule, removes the machinery it makes unnecessary, and closes those
seven entries.

### What changed for someone using Settings

- **Closing Settings waits for work still out (#5104).** Close or Escape asks nothing while a save, Test, Clear or
  Delete is still out, a switch's save included, and goes on once the helm answers. If the helm refused, Settings stays
  open on that page with the reason. If a replaced credential could not reach some hosts, Settings stays open on the
  Credentials page where those hosts are listed. Otherwise it closes, or asks about drafts still unsaved, and Discard is
  always offered. Recipe builds and deletes, leftover cleanup and Create SSH key carry on by themselves and do not hold
  Settings open. Nothing times out: if the helm never answers, Settings stays open, and any key press or click cancels
  the waiting close, so the person can close it again or carry on.
- **The Save button in the close question sends drafts one at a time (#5104)** and stops at the first refusal with that
  page shown. If you start something else while it runs (a switch, a Test), Settings stays open instead of closing over
  it. An unanswered recipe Delete confirmation goes away when that Save sends the recipe.
- **The credential editor waits (#5103).** Back and Cancel are unavailable while its save or Delete is out, so the list
  of hosts a replacement could not reach always shows on the Credentials page.

  One case is also fixed. Say you typed a new Tensorlake API key on the Credentials page and also had a credential open
  in its editor. You close Settings and choose Save, and the helm refuses the key. Credentials then shows the key with
  its error, and the credential editor sits behind a "Resume editing credential" button. Before, if you closed and saved
  again and the helm refused the credential, the error stayed hidden behind that button. Now the editor comes back to
  the front with its error.
- **The recipe editor waits (#5105).** In Add host, closing while a new recipe is being saved does nothing. When the
  save finishes, you are back in the Add host form, and Add host stays open. The recipe editor's "Save these changes
  before leaving?" question is never asked over a save, so it always offers Discard.
- **Create SSH key no longer polls (#5105).** When the helm takes longer than its reply limit, its message now says to
  select Create SSH key again in a few seconds. Selecting it again picks up the key that is still being made instead of
  starting another. Settings reads the Tensorlake account once after such a message, and says to try again if that read
  finds no key.
- **The close question names each custom agent draft (#5106)** by its name, and shows the affected page behind it only
  when every affected form is on that one page.
- **The page's Save and Discard act only on the page and drafts they showed (#5106).** A pointer press started on one
  page's Save and released after the page changed no longer saves the other page's drafts.

## Things you should know

- **How a waiting close goes on.** Each save, Test, Clear or Delete that Settings started tells the waiting close when
  it has finished, and only after its own screen updates are done, such as emptying a saved key's field. A preference
  save that something outside Settings started (a hint switch elsewhere, say) has no such signal of its own. For that
  case, Settings watches the app's list of operations in progress.
- **This may be more bookkeeping than needed.** The plan assumed the list of operations in progress could signal that a
  save had finished before the form had finished updating its screen. The #5104 reviewer found that in this version of
  the UI toolkit it signals after. A simpler design, which only watches the list of operations in progress, would behave
  the same and remove the per-form tracking in about five places. I kept the plan's design; simplifying it would need a
  fresh review of the plan's scope.
- **Short windows can hide the report of missed hosts.** When a replaced credential misses hosts while Settings is
  closing, Settings scrolls the Credentials page back to its top so the report shows. The closing editor then moves
  focus to its list's Add button. In a window too short for both, scrolling to that button wins. Ordinary saves that
  miss hosts already behave this way, so this is not new. Fixing it means changing where focus goes after the editor
  closes, which this plan does not cover.
- **No dialog-wide busy state.** As decided, nothing dims or locks while Settings waits. A Close pressed during a save
  does nothing visible until the helm answers.
- **Changelog fragments.** AGENTS.md asks for one with every fix, so #5103 and #5104 add one each, and #5105 and #5106
  add two each, one per change, although the plan's release note said none was needed.
- **The rebase.** The stack was rebased onto main before it was pushed. Main has moved since then, but only in the
  review feedback queue, which these PRs do not touch.

## Open questions and possible follow-ups

- **Which page shows behind the close question when both unsaved forms are on Credentials?** That is the Tensorlake API
  key typed on Credentials plus an open credential editor. The plan says to show the affected page whenever exactly one
  page is affected, so #5106 now shows Credentials behind the question. The TODO entry for naming custom agent drafts,
  which #5106 removes, had asked to keep the old behavior, which left the shown page as it was. That request was part of
  the planner's proposal, not of the triage decision, so I followed the plan. If you prefer the old behavior, it is a
  one-line change. I recommend keeping the new one, since the question names forms on that page.
- **Simplify how a waiting close goes on**, if you want less bookkeeping (see above).
- **Keep the missed-hosts report visible in short windows**, either by leaving focus where it is when the editor closes
  with hosts to report, or by scrolling back to the report after focus moves.
- **Date the low-latency helm rule.** The #5102 reviewer suggested also recording the rule under "Maintainer-confirmed
  decisions" in SPEC.md, which dates each entry. I did not, because the date you gave the rule is not recorded anywhere
  I could find. If you tell me the date, it is a short follow-up.

## PRs

- #5102: docs: record the low-latency helm rule for Settings saves.
- #5103: fix: keep the credential editor open until its save answers.
- #5104: fix: make closing Settings wait for work still out.
- #5105: fix: stop polling for the SSH key and let the recipe editor wait.
- #5106: fix: name custom agent drafts and bind the Settings footer (removes the seven TODO entries).

## Checks

Run now:

- GUI tests for Settings, the hosts views (Add host among them) and the session pane's trust band
  (`cargo nextest run -p farhelm-gui --lib`, tests under `settings::`, `hosts::` and `trust_band_tests`), on the rebased
  stack before #5106's last review fixes: 564 of 564 passed (recorded run 1b0dc49a-b27e-43bc-bfb4-db6c47333f1c).
- The same selection on #5106's final code: 567 of 567 passed (recorded run 32c3d27e-8e80-4351-b2fd-5932b7649466). Only
  a doc comment changed after it.
- The Settings screenshot comparisons in both palettes (`scripts/check-gui-goldens.py --scene 'settings-*'`) on the
  rebased stack: no differences (recorded run f3ebef1e-10cd-456a-a165-410ea1291d92). The screenshot of the old "close
  again later" note is retired along with the note.
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings` and `cargo fmt --check` on #5106's final code: clean.
- `dprint check` on the changed Markdown, `python3 releasing/check-changelog.py format`, and
  `python3 scripts/check-agents-md.py` (0 errors, one warning that was already there): passed.

Reused:

- The helm's Tensorlake tests for #5105's new timeout wording (`cargo nextest run -p farhelm-helm --lib`, tests under
  `tensorlake::`): 156 of 156 passed on #5105 before the rebase (recorded run 74bfbc22-7cd1-463a-95f8-31e3b8cb6c0d).
  #5105's later review fixes changed only GUI docs, GUI tests and changelog fragments, and the rebase brought in no
  change to that crate's Tensorlake code, so the coverage still applies.
- `cargo clippy` for the helm crate ran clean on the same revision, for the same reason.
- The screenshot comparisons ran before #5106's last review fixes. Those changed only tests and doc comments, so the
  coverage still applies.

Skipped:

- The rest of the GUI crate's tests, the other crates' tests, and the end-to-end and desktop batteries. The change is in
  Settings' own views, Add host's recipe editor and one helm reply text. The GUI tests above drive those through the
  real Settings dialog over the test stand-in for the helm, and the helm's Tensorlake tests cover the reply text.

## Review gate

Each PR had a fresh-context Opus 5.5 review at high effort with the plan's charter, plus the test-authoring checklist
when tests changed:

- **#5102:** two rounds. Round 1 found two medium and one low wording problems. Both mediums were fixed. The low one is
  the dated entry listed under follow-ups. Round 2 found nothing.
- **#5103:** two rounds. Round 1 found two low problems (the changelog overclaimed, and a test did not check its
  starting state) and three nits. All were fixed except one: a defensive guard stays without a direct test. Round 2
  found nothing.
- **#5104:** three rounds. Round 1 found four medium problems, all fixed:
  - the close question's Save could close Settings over work started while it ran;
  - a recipe Save did not dismiss an unanswered Delete confirmation;
  - a sentence in SPEC_impl.md was stale;
  - two paths had no tests: the one for saves started outside Settings, and input cancelling a waiting close.

  It also found six low problems, all fixed except one extra test the code cannot reach, and made the design note above.
  Round 2 found two low problems: a test comment was fixed, and the short-window scrolling became a follow-up. Round 3
  found nothing.
- **#5105:** three rounds. Round 1 found no bugs, and five low problems and four nits about docs, a test and the
  changelog, all fixed. Round 2 found one low changelog wording problem and two nits, all fixed. Round 3 found nothing.
- **#5106:** three rounds. Round 1 found three medium problems: two tests would pass without the fix they pinned, and
  the "act only once shown" rule had no test. It also found four low problems. All were fixed except two tests that
  cannot be set up from the UI. Round 2 found three low problems and two nits, all fixed except one optional test. Round
  3 found nothing.

### Landing

#### What landed with it

This plan was the only one in the round. Since the executor last rebased its stack, main gained a TODO entry (about
terminal tabs overflowing the New terminal button) and records in the review feedback queue. While the round's checks
ran, main also gained a record of an investigation into the desktop app's idle CPU use and planning edits to TODO.md.
None of it touches Settings, so nothing interacted. The report says main moved only in the review feedback queue; that
missed the new TODO entry, which survives the landing next to the seven entries the stack removes.

An independent reviewer and I read the stack against main. The reviewer found nothing that would be wrong on main:

- every kind of work that makes a close wait eventually lets it go on, including a form that goes away mid-save and a
  lost connection;
- a switch's refusal is recorded before anything can let the close go on, so Settings never closes over it;
- #5103 removed a check that ignored an answer from a credential editor that was no longer the open one. It is no longer
  needed: Add is unavailable while an editor is open, and Back and Cancel are unavailable while its save or Delete is
  out, so an answer always comes back to the editor that sent it. This is not the check the report says stays without a
  direct test of its own;
- the new tests fail on the old code;
- nothing is left of the removed machinery: the "close again later" note and its screenshot, the close question that
  reopened itself, the half-minute SSH key check, and the old way the page's Save and Discard acted on whichever page
  was current when the click ended;
- the TODO change removes exactly the seven entries the report names.

#### Fixes made while landing

- **SPEC_impl.md overstated how a waiting close goes on.** It said each operation Settings started lets the close go on
  through its own completion, and that the app's list of operations in progress does so only for a preference saved from
  outside Settings. For Settings' own switches, that list usually reports the save finished before the switch's own
  signal does. Each switch records a refusal at once, so either signal is safe. The sentence now says so. No behavior
  changed.
- **The changelog draft for naming custom agent drafts undersold the change.** It now also says that a single custom
  agent draft is named, where before the question said only "Custom agent". It also says that Settings now shows
  Credentials behind the question when the unsaved forms are a Tensorlake API key typed there and an open credential
  editor.

#### What the landing made untrue in the report

- **The suggested simplification would not behave the same.** The report says a design that only watches the app's list
  of operations in progress would behave the same and drop the per-form tracking. It would not: Send test for
  notifications and the Tensorlake key's Test never appear in that list, so a close waiting on a Test would never go on.
  A simpler design would also have to hear from those two forms. Take this into account before asking for that
  follow-up.

#### Worth knowing

- **Two problems from before this plan remain.** Both are already in the review feedback queue, but their entries cite
  code this plan removed, so read them against the new behavior at triage:
  - the close question's Save still closes Settings when it finishes, even if an inline Delete question for a leftover
    Tensorlake sandbox or container is still unanswered, because those Deletes do not make a close wait and the Save
    does not check whether the person moved on. A cheap fix would remember when Save was chosen and stay open if the
    person did something else since;
  - Discard in the close question still does nothing visible when a draft changed while the question was open.
- **The waiting close's bookkeeping is about 250 lines.** The reviewer sketched a smaller design that also hears from
  Send test and the Tensorlake key's Test, so it avoids the gap above. The plan's decision on the order in which a
  form's updates and the close happen prescribes the current shape, so changing it is your call, and I have not added a
  TODO entry.

#### Checks

Run once, on the stack rebased onto main as it stood when the landing started. The SSH test worker could not be reached
this round, so after ten minutes the slow suites ran on a boat sandbox (an x86_64 Ubuntu 24.04 VM) instead, and clippy
ran locally:

- Locally, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
  `cargo clippy -p farhelm-desktop --all-targets` without and with `--features agent`, all with `-D warnings`: clean.
- On a boat, the GUI crate's and the desktop app's tests and the helm's Tensorlake tests (a nextest archive of
  `-p farhelm-gui -p farhelm-desktop -p farhelm-helm` filtered to those, run through the recorder in the mode that takes
  an archive, which keeps passing tests' output in nextest's report), recorded run bb6cf954-7598-4683-b575-c28e9c61a3b1:
  3,888 passed, none failed, and no runtime SKIPPED lines. The 1,812 tests nextest reported as skipped are the helm's
  other tests, which the filter left out. This is the whole GUI crate, where the executor ran only the Settings, hosts
  and trust band tests.
- On a boat, the GPUI end-to-end smoke (`scripts/gui-e2e-smoke.py`, which opens and closes Settings), recorded run
  71fc87bf-c54e-466c-9f13-e8bdf97ff3df: 49 steps passed, with the paste step skipped as on every Linux run.
- Locally, the full screenshot comparison (`scripts/check-gui-goldens.py`), recorded run
  a11a4602-5afe-4956-9521-0aedc35de1a4, and the screenshot batch's scene list test, recorded run
  f2a9f968-033b-43f1-8a4a-4766bad19130: passed, so the retired screenshot left no reference behind.
- Locally, `cargo fmt --all -- --check`, the test-sleep check, `python3 releasing/check-changelog.py format` and
  `dprint check` on the changed Markdown: passed, and the changelog lint and `dprint check` again after the landing's
  wording fixes.

Reused: nothing. Skipped: the end-to-end suite and the other crates' tests, since the change is in the desktop app's
Settings and Add host views and one helm reply text.
