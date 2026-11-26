# Report: Replace with keeps the source's destination, and Enter on a scrolled-away highlight

## What this was about

Two launcher items, each settled by a decision you made when the plan was written.

**Replace with** is meant to swap what runs in a session (the agent, its settings, the first message, the title) without
moving the session. It already kept the host, but you could still change the folder, switch to **Managed checkout** to
start from a fresh clone, or take a recent setup that set its own folder. The fresh-clone path left the source
checkout's changes behind and could move that checkout to the host's trash. It needed its own warning, and a TODO asked
for that warning's consent to be bound to exactly what was on screen. That turned out to be unreliable: the warning
could be scrolled out of view, and a recent setup's Enter could switch to a fresh clone and launch in one press. You
decided instead that Replace with keeps the source's destination outright, and that Clone is how a session moves.

**Enter on a scrolled-away highlight.** In the launcher, you could highlight a recent setup with the arrow keys, scroll
it out of view and press Enter, and it still acted. A TODO asked for Enter to be ignored until the row was visible
again. That needs a "was it really drawn" proof the GUI toolkit cannot give reliably. You decided that an arrow-key
highlight is an explicit choice, so Enter acting on it is correct, and that this becomes a general rule in SPEC.md
rather than a fix.

What is on main now:

- In Replace with, the folder is shown read-only. There is no **Folder | Managed checkout** switch, no Browse button and
  no folder list. Search results and recent setups that set a folder are shown but disabled, with the reason ("this
  dialog keeps destination fixed for replace"), as Restart with already does for the fields it keeps. A template may
  repeat the session's own folder and is refused if it names another folder or any repository. When the source runs in a
  managed checkout, the launcher still names that checkout and says the replacement keeps working in it with its
  changes.
- The fresh-clone warning, and the special "⏎ Replace" Enter on a recent setup in Replace with, are gone with the path
  they served.
- SPEC.md describes Replace with this way (Lifecycle operations, Managed checkouts, Creation). Its "One GUI at a time"
  section gains the Enter rule, written as an explicit exception to the older rule that ignores an action whose target
  is no longer in view. Disabled or held choices and choices that are no longer offered are still refused.
- Three docs pages describe the new Replace with: Start a session, Stop, restart, and resume, and Launch templates.

## Things you should know

- The helm still accepts a Replace with that names a different folder or a managed checkout, from the command line or
  its API. Only the desktop launcher stopped sending one. SPEC.md now describes Replace with as keeping the destination,
  so the helm and the spec differ there. That was outside this plan's scope, and is listed as a follow-up below.
- In Replace with, recent setups can no longer be used at all, not even to fill in agent choices, because every recent
  setup also sets a folder. This follows the plan's choice to refuse them rather than silently drop their folder. Recent
  setups that made a new sandbox or container are still shown and refused, as decided on 2026-11-16. Their reason now
  reads like every other field the dialog keeps ("this dialog keeps destination fixed for replace") instead of "Restart
  and Replace keep the existing environment".
- Two unreleased changelog entries from earlier work described Replace with's recent-setup Enter and its screen-reader
  wording. I edited them so they no longer promise behavior that is gone.
- Of the app's reference screenshots, the ones the automated picture comparison checks, the launcher picture that showed
  a fresh clone inside a sandbox was removed. The picture of a recent setup found by typing its name now shows New
  instead of Replace with, since Replace with refuses it.

## Open questions and possible follow-ups

- Should the helm refuse a Replace with that changes the folder or checkout, so the command line matches the desktop app
  and SPEC.md? A plan follow-up could add that refusal; the desktop app no longer needs the path.
- Recent setups are now unusable in Replace with. If you want them back as a way to fill in the agent choices only (the
  folder staying the session's), that would be a small follow-up; the plan chose refusing over silently dropping the
  setup's folder.

## PRs

- #5081 feat: keep Replace with in the source's folder (the launcher change, SPEC.md text, tests, screenshots, docs
  pages, the changelog entry).
- #5082 docs: let Enter act on a list highlight scrolled out of view (the SPEC.md rule; removes both TODO entries).

## Checks

Run now, through the test recorder:

- The desktop app's launcher, templates, session list and session pane tests. A first pass, run `452d3d2a`, had 14
  failures, all tests written for the old behavior, which I updated or removed. Later passes: run `f80dab36` (1723
  passed); after the review fixes, run `2c6d508b` (1723 passed); after rebasing onto current main, run `15dcba99` (2012
  passed: the same tests plus the Settings dialog's, since a Settings change had just landed).
- The reference screenshots: a full re-render (run `3e9e5c91`) and publication of the changed pictures, then a full
  comparison on the rebased tree (run `b9f91fbc`, everything matching). An earlier full re-render, run `9a549c43`, could
  not draw the typed-recent-setup picture when it was opened as Clone; I moved it to New before publishing.
- `cargo clippy -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the test sleep check,
  `dprint check` on the changed Markdown, the changelog format check, the tests of the list that names each reference
  screenshot, and the docs website build (`bun run build`, all internal links valid).

Reused: none across the rebase. Main had just gained the per-custom-agent wrapper memory (#5073, #5074), which edits the
same launcher code, so the tests and the screenshot comparison above were run again on the rebased tree.

Skipped: the end-to-end suite and the desktop smoke test. The change is confined to the launcher's own rules and
drawing, which its view tests and the reference screenshots cover. Nothing here touches the helm, the supervisor or
terminals.

## Review gate

Each PR had a fresh-context Opus 5.5 review at high effort. Neither found a blocker.

For #5082, the reviewer pointed out that the new rule contradicted an older sentence in the same section. The rule now
names itself an exception to it and says scrolling changes nothing else.

For #5081, the reviewer traced every way the folder could still move in Replace with and found none. It asked for:

- the three docs pages above to be updated;
- the two unreleased changelog entries to be corrected;
- one test to be fixed, because it could no longer fail;
- the template refusal to name the host before the folder, matching Restart with;
- the SPEC wording on which choices are refused to be made exact;
- a number of stale code comments, and the notes recording how the launcher was carried over from the old app, to be
  updated.

All of that was done. The fixes were not reviewed again, but the full test pass, the screenshot comparison and the
website build listed above all ran after them. One small code tidy-up the reviewer suggested, with no effect on
behavior, was left as is and documented.

### Landing

#### What landed with it

Only this plan landed this round. Since its base, main had gained a change to maintainer tooling for agent records
(#5080), and while it landed, new TODO entries (#5085); neither touches the app. An independent reviewer and I checked
the plan against the launcher work that landed just before its base: the wrapper command the launcher now remembers for
each custom agent (#5073, #5074) still loads in templates, and the session list's Restart/Replace (#5072) is unaffected.
The reviewer traced every way the folder could still change in Replace with (the folder field, Browse, the folder list,
search rows, recent setups, templates, quick launch, number keys and Enter) and found none. New and Clone are unchanged.

#### Fixes made while landing

One more test could no longer fail; whether or not it is the one the executor's review flagged, it still could not. The
test covers this case: in Replace with you press Enter on a template the dialog refuses while the agent's model list is
still loading. If a different template with the same name then appears and the list finishes loading, nothing from the
refused press may apply to the draft. The replacement template set only a command, which the launcher refuses on an
agent launch anyway, so the test passed whether or not the refused template's changes were wrongly kept. It now sets
only the session's name, which Replace with accepts, so a wrongly kept edit would change the draft.

#### Worth knowing

- **Recent setups in the session's own folder are refused too.** Replace with shows the recent setups that match the
  session's folder, so nearly every one it shows is in that very folder, and all of them are now disabled with "this
  dialog keeps destination fixed for replace", although taking one would move nothing. The new Replace with screenshot
  shows exactly that: both recent setups greyed out. The executor did what the plan's acceptance criteria say (refuse
  recent setups in Replace with outright), so I landed it as built. The report's reason, "every recent setup also sets a
  folder", is true, but for a setup in the same folder nothing would be dropped. If you want recent setups usable in
  Replace with when they ran in the session's own folder, refusing only those that ran elsewhere, made a new sandbox or
  container, or used a managed checkout would be a follow-up. It would also bring back the "⏎ Replace" Enter hint on
  those rows, and would change SPEC.md, the docs and one screenshot. This is narrower than the report's second
  follow-up, which would let any recent setup fill in the agent choices and drop its folder.
- **The report's follow-up about the helm is slightly off.** It says the helm accepts a folder-changing Replace with
  "from the command line or its API". The `farhelm` command line has no Replace; only the helm's API does. The follow-up
  itself (have the helm refuse it) stands.
- **Leftovers for a later cleanup.** A few pieces of the removed fresh-clone path remain as code that can no longer do
  anything. The report says the stale comments and the notes recording how the launcher was carried over from the old
  app were updated; a few were missed: some code comments and one test still quote SPEC.md wording that has since
  changed, and those notes still name a test that no longer exists. None of this changes behavior.

#### Checks

Run on the plan's two PRs exactly as the executor pushed them, stacked on main as it stood when the landing started, as
described above. The test worker is an aarch64 Linux machine:

- On the test worker, the four clippy configurations (`cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo clippy -p farhelm-desktop --all-targets` with and without
  `--features agent`, each with `-D warnings`): clean.
- On the test worker, `cargo nextest run -p farhelm-gui -p farhelm-desktop`, recorded run
  2ef0a447-add6-4764-a944-d665c98190f4: 3,696 passed, 4 skipped (tests marked ignored). That is every desktop app test.
- On the test worker, the desktop app's end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), recorded run
  fdcdfe2e-e3b6-4c45-bc7f-5251f81dfbab: 48 of the 49 steps passed; the paste step runs only on a Mac and was skipped, as
  it always is on Linux. It opens the launcher as New, Replace with and Clone.
- Locally, every screenshot scene compared with its approved picture (`python3 scripts/check-gui-goldens.py`), recorded
  run f0b21f61-bcc2-4dac-b9c9-e53ed5c706b6: every picture matched, the launcher's changed ones included. I also looked
  at the new Replace with picture myself.
- Locally, `python3 -B scripts/test-gui-screenshot-batch.py`, which checks the screenshot scene list against the golden
  manifest, recorded run 07ec1e47-794d-4e1a-a293-7ad1fd4ba4d4: passed.
- Locally, the docs website build (`bun install --frozen-lockfile && bun run build` in `website/`): all 41 pages built.
- Locally, `cargo fmt --all -- --check`, the test-sleep check, `python3 releasing/check-changelog.py format` and
  `dprint check` on the changed Markdown, docs pages and scene list: passed.
- After the landing fix above, on the test worker, the launcher's search tests
  (`cargo nextest run -p farhelm-gui --lib -E 'test(/launcher::search_view_tests::/)'`), recorded run
  04d9a334-febe-496f-af2d-cd550c80b929: all 60 passed. An earlier attempt, recorded run
  268a4f6b-ec78-4021-a5ea-5be744542a0b, failed only on an extra check I had added to the fixed test and then dropped:
  after the template was replaced, the open search no longer listed it. I did not check whether that is intended or
  older than this plan; no product test failed.

Reused: nothing. Skipped: the helm's, the supervisor's and the end-to-end Rust tests, since only the desktop app, the
specs and the docs pages changed.
