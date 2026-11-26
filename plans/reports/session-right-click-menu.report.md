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
