### What this was about

A session's actions were reachable only through the small ⋯ button at the end of its row, which shows on hover, and a
batch of marked sessions could be acted on only from the marked bar at the top of the list. You asked for right-click to
open a session's menu, and for right-click on a marked session to open a menu that acts on all the marked ones. Your
decisions: a right-click on an unmarked session while others are marked opens that session's own menu and leaves the
marks alone; the ⋯ button stays; the menu opens at the pointer, kept inside the window; Shift+F10 opens it beside the
row. This plan ran after the "Start several selected sessions at once" plan had landed, so the combined menu includes
Restart/Replace.

What you get now:

- A right-click anywhere on a session row (its title, status dot, bell and pin grip included) opens that session's menu
  at the pointer, with the same commands as its ⋯ button. A right-click on a group header opens the group's menu.
- Shift+F10 while focus is on a row, a group header or a control inside one opens its menu beside the row.
- A right-click while another row's menu is open moves the menu to the new row in one click.
- With two or more sessions marked, a right-click on a marked one opens the marked menu with Restart/Replace, Stop and
  Delete (each only when the marked bar offers it) above the pin, group, mute and read actions. Each acts exactly as the
  bar's button does. That menu closes by itself when what it offers changes, or when fewer than two marked sessions are
  shown.
- A right-click on an unmarked session, or on the only marked one, opens that session's own menu.

### Things you should know

- Closing a menu opened by right-click or Shift+F10 puts focus back where it was before, usually the terminal, as
  SPEC.md's "Keyboard focus" rule asks. A menu opened from the ⋯ button still returns focus to the button. The same goes
  for a command chosen from the menu, Rename among them.
- One exception, a judgment call: the Stop or Delete question a marked right-click menu asks returns focus to the marked
  bar on Cancel, exactly as when the same question is asked from the bar, since it is the bar's question about the
  marked sessions (see the open question below).
- In the marked right-click menu, Restart/Replace is greyed out while a host change is in flight, with the reason as its
  description; the bar's own button shows busy instead, which a menu item cannot.
- The bar's ⋯ menu is unchanged: it still offers only the pin, group, mute and read actions, since Stop, Restart/Replace
  and Delete are buttons right beside it.
- No drawing of existing screens changed, so no screenshots were re-taken. SPEC.md's "Session list" gained a paragraph
  describing all of this, and the PR adds a changelog draft.

### Open questions and possible follow-ups

- Where should Cancel on a Stop or Delete question asked from a marked right-click menu put focus? I kept the bar's
  behaviour (focus goes to the marked bar). The alternative is to return it to where it was before the right-click,
  usually the terminal, which matches how closing the menu itself behaves.
- A right-click on a session's bell while its notifications list is open opens the session's menu on top of that list
  instead of replacing it, and a right-click in the middle of dragging a pin, or of a Shift-drag marking sweep, opens a
  menu mid-drag. Both are rare, and each fix needs the bell's or the drag's own state to reach the row, so I left them.
  A follow-up could close the bell's list on a right-click and ignore right-clicks while a drag is under way.

### PRs

- #5090 feat: open a session's menu with a right press

### Checks

Run now (the tests through the test recorder):

- Twelve new GUI tests of the right-click and Shift+F10 menus, and one unit test that a menu opened at the pointer stays
  inside the window; all are in the run below and passed there. The tests the second review found weakest were checked
  by disabling the rule they pin: both then failed at the intended assertion (run 12dd7e2e).
- The session list, the layer manager (which stacks the app's menus and dialogs) and the shared component GUI tests
  (`cargo nextest run -p farhelm-gui --lib -E 'test(/^session_list::/) | test(/^layers/) | test(/^components::/)'`): 829
  passed (run c1d91516), and again on the rebased PR head (run d5e28fce).
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the test delay checker (306
  delays inspected, none lacking a rationale), `dprint check` on the changed Markdown, and
  `python3 releasing/check-changelog.py format`: all clean, on the rebased PR head.

Reused: none. main had moved by several PRs (the launcher's Replace with among them, which the row menu also offers);
the rebase applied without conflict and every check above ran again on the rebased head.

Skipped: the GUI screenshot comparisons, since no existing screen's drawing changed, and the full workspace battery,
since the change is confined to the desktop app's session list and its layer manager.

### Review gate

Two rounds, each a fresh Opus 5.5 reviewer at high effort with the plan's general charter. The first found two real
problems: a marked right-click menu was not recorded as its row's open menu, so a click on that row's ⋯ stacked a second
menu on it; and right-click menus returned focus to the ⋯ button instead of to where focus was. Both are fixed, along
with that review's smaller points about tests and docs. The second found that a right-click on a marked row's ⋯ while
that row's own confirmation question was open opened the marked menu over the question and lost track of it. That is
fixed with a test, and one test was rewritten because it passed without the rule it was meant to pin. The gate passed
after the second round.

### Landing

#### What landed with it

This round landed four plans, this one first: then launcher-late-wake-failures (the launcher reports a sandbox or
container's late wake failure at once), supervisor-stop-pending-launch (Stop refuses a session whose start was never
confirmed) and drift-updater-teardown (the drift tests' agent upgrades run in their own systemd scope). They change
separate code and met only in TODO.md, where the landing kept every plan's removal. Since this plan's base, main had
gained only two supervisor and helm changes, neither in the desktop app. Two independent reviewers and I read the round
together. The right-click menu for several marked sessions remembers which sessions it was opened for, separately from
the marked bar, so acting from one cannot use what the other showed earlier. Its Stop and Restart/Replace send their
requests the same way the bar's buttons do, so a session whose start was never confirmed, which Stop now refuses, shows
the usual refusal line on its row, whichever one you used. A right-click on a row still closes any other open menu, such
as a host's.

#### Fixes made while landing

None for this plan. Neither independent reviewer found anything to fix here; the round's one landing fix was in
launcher-late-wake-failures (see its notes).

#### Worth knowing

- **The marked menu repeats the marked bar's rules.** The rules for which bulk actions to offer, when each is
  unavailable and what its help says exist twice, once for the bar and once for the menu. A reason added to the bar
  later would be missing from the menu unless someone adds it in both places. A follow-up could make both use the same
  rules.
- **The order differs from the bar.** The right-click menu lists Restart/Replace, Stop, Delete, while the bar shows
  Stop, Restart/Replace, Delete. All three often show together, for example when every marked session has ended.
- **Focus after a bulk question asked from the menu.** Cancel returns focus to the bar's ⋯ button rather than where it
  was before the right-click (usually the terminal). The report already raises this as an open question.
- **Shift+F10 on a marked row opens the marked menu**, but SPEC.md says so only for a right-click, and the docs site's
  session list page does not mention right-click yet.

None of these blocks. The first two would be small code follow-ups if you want them, the third is the report's open
question, and the fourth is a small docs follow-up (a SPEC.md sentence for Shift+F10 and a right-click mention on the
docs site's session list page).

#### Checks

Run once for the whole round, on its four plans stacked in queue order (session-right-click-menu,
launcher-late-wake-failures, supervisor-stop-pending-launch, drift-updater-teardown) on main as it stood when the
landing started; what main gained since each plan's base is in each plan's notes. The test worker is an aarch64 Linux
machine with a systemd user manager:

- On the test worker, the four clippy configurations (`cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo clippy -p farhelm-desktop --all-targets` with and without
  `--features agent`, each with `-D warnings`): clean.
- On the test worker,
  `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor -p farhelm-gui -p farhelm-desktop`: the
  first attempt, recorded run b6571c52-9267-4c9b-9635-f7c159c2a3f1, never ran a test, because the compiler was killed
  while building the helm's tests (the worker ran out of memory with four build jobs). The rerun with two build jobs,
  recorded run 264e5b64-065f-46f0-8dcb-ab15f3ef799d: 7,624 passed, 17 skipped (tests marked ignored). That is every test
  of the shared message types, the helm, the supervisor, the desktop app and its GUI.
- On the test worker, the end-to-end tests of host creates, host terminals, relaunches, supervisor restarts and every
  test whose name contains `restart` or `stop` (`cargo nextest run -p farhelm --test e2e` filtered to them), recorded
  run 08bd628d-e968-4273-8466-14c3e4f84735: 160 passed, and none skipped for lack of systemd.
- On the test worker, the desktop app's end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), recorded run
  2a146ce3-50ae-460e-a84b-d597d7d7b659: 48 of the 49 steps passed; the paste step runs only on a Mac and was skipped, as
  it always is on Linux.
- Locally, every screenshot scene compared with its approved picture (`python3 scripts/check-gui-goldens.py`), recorded
  run 33bc1d03-4b78-4cc2-a05e-d822d0dce764: every picture matched, the new late wake failure picture included.
- Locally, `python3 -B scripts/test-gui-screenshot-batch.py`, which checks the screenshot scene list against the golden
  manifest: recorded run b81cb326-fd3c-4921-9f49-69ff01bb3f0d failed because the new late wake failure picture was
  missing from the scene list; after the landing fix in launcher-late-wake-failures, recorded run
  ffecfaab-79e4-4bfd-942e-a25925a47bd5 passed.
- Locally, the drift runner's own tests (`python3 -B scripts/test-run-harness-drift.py`), recorded run
  029c1d18-8368-4a93-b71c-ea765289da54: passed, on this machine's systemd user manager.
- Locally, `python3 scripts/check-agents-md.py` (0 errors; its one warning is an old one about the frozen triage
  archive), `cargo fmt --all -- --check`, the test-sleep check, `python3 releasing/check-changelog.py format` and
  `dprint check` on the changed Markdown: passed.

Reused: nothing. Skipped: the Podman suites, since no plan changes how containers start or stop; the real-agent drift
cases themselves, which spend vendor turns and run only on request.
