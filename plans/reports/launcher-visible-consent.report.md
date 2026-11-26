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
