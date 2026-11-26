### What this was about

After a remote host reboots, every session on it has ended (Interrupted, or with its terminal gone), and starting them
again meant opening each session and pressing Restart or Replace one at a time. The TODO entry "Start several selected
sessions at once" asked for a way to start all the marked sessions together once they have all stopped. You decided up
front: cover every ended state plus a missing terminal; one action labelled `Restart/Replace` whose hover help says
"Restart when possible, Replace when not"; place it beside Stop in the marked-sessions bar (Stop stays, since ended
sessions can still have open shell tabs); and run it on the click with no confirmation.

What was built: when every marked session shown (retired container sessions aside) has ended or lost its terminal, the
marked bar shows `Restart/Replace` between Stop and Delete. One press restarts each session that can resume its
conversation and replaces the rest, using the same per-host queue bulk Stop and Delete use: one request at a time per
host, hosts in parallel, each queued row held as pending, refusals on their own rows, and the batch's marks cleared when
it finishes. A session's row in the list now also shows "Restarting on" with the host's name and the seconds waited once
a Restart has taken two seconds, as it already did for Replace, wherever the Restart came from.

### Things you should know

- **A Replace that would end something still running is left out, not replaced.** A Replace makes the new session before
  it deletes the old one. If the old one turns out to have a running program or an open terminal tab at that moment, the
  delete is refused and both sessions stay, as SPEC.md already specifies for every Replace. So a batch Replace cannot
  count on that refusal to protect an open tab: it would leave a duplicate session behind. With confirmations on (the
  default), a session whose conversation cannot be resumed and that still lists a terminal tab (or whose agent may still
  be running beside a missing terminal) is skipped, and its row says "Replace: not started, its program or a terminal
  tab may still be running, and Replace would end it. Use Replace from the session's menu to confirm." With Skip session
  confirmations on, it is replaced like the others, closing the tab, exactly as its own Replace would do then. This
  keeps your "no confirmation" decision without letting one press close tabs nobody was told about. SPEC.md says so in
  "Session list" and in the Replace bullet of "Lifecycle operations".
- **What else is left out, and how it says so.** A retired container session is skipped silently and keeps its mark (as
  bulk Stop treats it). A session whose sandbox or container is suspended, stopped or changing says "not started, its
  environment is suspended, stopped or changing. Resume it, then try again."; one being deleted gives the existing
  "being deleted" reason. A busy session gets the usual "not started, another operation was in progress. Try again."
  While a host is being added, set up, updated or removed, the button waits with "Wait for the host change to finish",
  and a row whose turn comes during one says the same and is not sent. A row that changed while it waited for its turn
  so that it would now get something else (it became resumable, or a terminal tab opened) is not sent and says "not
  started, the session changed while it waited for its turn. Try again." Sessions on a host the helm cannot reach are
  still sent, as the open session's header does, and the helm's refusal shows on the row.
- **The button can show but be unavailable.** When every marked session would be left out (for example all of them have
  open tabs and cannot resume), the button is drawn unavailable with a hover help saying why, as Stop and Delete do when
  every environment they would act on is being deleted.
- **A second press, or a double click, does nothing to the rows already starting.** Without this, the second batch would
  have left a false "another operation" line on each row and cleared the marks early.
- **Focus.** A keyboard press moves focus to "N marked", since the button disappears from the bar as soon as one
  restarted session lists as running. A mouse press leaves focus where it was.
- **The release note.** The plan's release note calls the button "Restart"; it is labelled "Restart/Replace", as your
  decisions settled. The changelog fragment tells the release curator so.

### Open questions and possible follow-ups

- If skipping sessions with open tabs proves tedious after a reboot, the batch could instead ask once, naming the tabs a
  press would close, and then replace those sessions too.
- No screenshot test shows the new button: the existing marked-bar screenshot marks two live sessions, so the button is
  not drawn there and no picture changed. A screenshot with ended sessions marked would pin its look.
- No test checks on its own that a replacement which opens late in a batch does not take focus after you have moved
  elsewhere since the press. The code carries the moment of the press through to each queued Replace, so this cannot be
  forgotten silently, and the existing Replace tests cover what Replace does with it. Worth adding if that focus rule
  ever regresses.

### The PRs

- #5065 `feat: show restart progress on session rows`: the row's "Restarting on" wait line; SPEC.md's Restart bullet now
  lists the session row alongside the header and the interrupted card as places that show the wait.
- #5072 `feat: restart or replace marked sessions together`: the button, the batch, SPEC.md, the tests, the TODO.md
  entry's removal and the changelog fragments.

### Checks

Run now, on the rebased stack: the desktop app's tests of the session list's bulk actions, marking and lifecycle
actions, and every desktop app test matching restart, replace, wait or progress, through the recorder (run
09695ff8-6bca-4bac-8ee6-759746549216, 431 passed):

```
cargo nextest run -p farhelm-gui --lib -E 'test(/session_list::(bulk_start_tests|bulk_tests|marking_tests|lifecycle_tests)::/) | test(/restart|replace|wait|progress/)'
```

Reused, from the final code before the rebase onto the latest main, whose only code changes since were a supervisor-side
Restart with fix and a text-field paste fix, neither touching the session list:
`cargo clippy -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and the test-sleep check (306
delays, none unannotated). Run on the rebased stack: `dprint check` on SPEC.md and TODO.md, and
`python3 releasing/check-changelog.py format`. Earlier recorder runs, superseded by the run above and not relied on:
ef9e33db-6fd4-4876-8942-23883d52a0bc (430 passed), 35dc53b9-c824-45b2-ae5d-8a4fd1398c96 (80 passed), and #5065's own
tests of the Restarting wait line, cd2bce76-fd15-4662-8e40-2f0e28ab4c73 (149 passed).

Skipped: the workspace-wide Rust run and other crates' tests, since only the desktop app's session list changed and
nothing outside it depends on what changed; the desktop app's screenshot tests, since no screenshot draws the new button
and the marked bar's existing one is unchanged; the desktop app's end-to-end smoke test, whose marked-bar steps do not
cover bulk actions.

### Review gate

Each PR was reviewed by a fresh-context Opus 5.5 agent at high effort. #5065's review found an unused import that would
have failed lint, a stale SPEC sentence and docstrings, and a missing test for an unselected row's announced wait; all
fixed. #5072's review agreed with the Replace-safety rule and the shared queue design, and found nine issues: a queued
Replace counting "moved on" from its turn rather than from the press, SPEC.md contradicting itself on Replace's
confirmation, the button offered when nothing could start, a row not re-checked at its turn, a queued row's count of
seconds waited able to start from an earlier request, focus dropping when the button disappears, a second press, test
gaps, and nits. All were fixed, and a verification pass by the same reviewer confirmed the fixes. It left one low issue
(a mouse press moved focus), fixed afterwards and not reviewed again, and the focus test listed under follow-ups, which
was not added.

### Landing

#### What landed with it

This round landed this plan, then custom-agent-delete-prompt (the Settings custom agent Delete question). The two change
different parts of the desktop app and met only in TODO.md, where the landing kept both removals. Since this plan's
base, main had gained the fix that remembers each custom agent's wrapper separately for each registration (#5073,
#5074); while it landed, main also gained a fix to the release brick test's own harness (#5076). An independent reviewer
and I read these against this plan. A bulk Restart sends the same request the row's own Restart does, and reads nothing
that the recent fix applying "Restart with" settings before the agent reports (#5071) changed. Replace has never
recorded a custom agent's wrapper, alone or in a batch, so the bulk Replace adds nothing the wrapper fix would need to
cover. Nothing interacted.

#### What the new button looks like

No screenshot test draws the button, so I rendered the marked-sessions screenshot scene once with an interrupted and an
exited session marked, in light and dark. This was a local, uncommitted change, reverted afterwards. "Restart/Replace"
draws legibly between Stop and Delete in both. It is wide enough that, at that scene's sidebar width, the bar wraps: the
marked menu (…) and Clear (×) move to a second line under "2 marked", Stop, Restart/Replace and Delete. With live
sessions marked the button does not show, and the bar stays on one line as before. If the second line bothers you, a
shorter label would keep one line; that would be a follow-up, since the label is the one you chose.

#### Fixes made while landing

None. The independent review found nothing to fix in the code; the one wording gap it found (focus, below) is left for a
later SPEC.md edit.

#### Worth knowing

- Where focus goes after a press. The fix made after the last review moves focus to "N marked" only when the button
  itself had focus at the press, so that focus is not lost when the button disappears once a restarted session runs. A
  keyboard press always qualifies. On Linux a mouse click also gives the button focus first, so a click moves focus to
  "N marked" too, and a test checks that; on a Mac a click does not focus a button, so focus stays where it was. Nothing
  has run on a Mac. The report's "a mouse press leaves focus where it was" holds only on a Mac, and SPEC.md mentions
  only the keyboard press. Possible follow-up: have SPEC.md describe the rule as "when the button had focus".
- With Skip session confirmations on, a session whose terminal tab closes while it waits its turn in the batch is left
  out as "changed while it waited", although replacing it would now be safer. Rare, and pressing again starts it. No
  follow-up proposed.

#### Checks

Run once for the whole round, on bulk-start-sessions and custom-agent-delete-prompt stacked in that order on main as it
stood when the landing started (`bf468652`); what main gained since each plan's base is in each plan's notes. The brick
test harness fix (#5076) that landed during the landing is exercised by no check here. The test worker is an aarch64
Linux machine:

- On the test worker, the four clippy configurations (`cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo clippy -p farhelm-desktop --all-targets` with and without
  `--features agent`, each with `-D warnings`): clean.
- On the test worker, `cargo nextest run -p farhelm-gui -p farhelm-desktop`, recorded run
  bd36cea5-d4a2-4942-86ca-981f0eee85fb: 3,692 passed, 4 skipped. That is every desktop app test, wider than either
  executor's selection.
- On the test worker, the desktop app's end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), recorded run
  25abe428-bee0-437b-9486-34e05898acf1: all 49 steps passed, except the paste step, which runs only on a Mac and was
  skipped as always on Linux. It marks no sessions, but it exercises the row menus' Stop, Restart and Replace and
  Settings opening and closing.
- Locally, every screenshot scene compared with its approved picture, using both plans' scene lists together
  (`python3 scripts/check-gui-goldens.py`), recorded run 9bb96428-674a-4cb0-98e1-6be02b626c7e: every picture matched,
  the new custom agent Delete scene included.
- Locally, `python3 -B scripts/test-gui-screenshot-batch.py`, which checks the screenshot scene list against the golden
  manifest, recorded run 26eeb3f3-b1a9-43bc-a31e-63b8e02b4697: passed.
- Locally, `cargo fmt --all -- --check`, the test-sleep check, `python3 releasing/check-changelog.py format` and
  `dprint check` on the changed Markdown and the scene list: passed.

Reused: nothing. Skipped: the helm's, the supervisor's and the end-to-end Rust tests, since both plans change only the
desktop app and nothing it sends.
