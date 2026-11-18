## What this was about

When a background session starts a turn, it can rise in the session list. When it finishes, it can turn blue and play a
sound. Mute keeps that routine work from drawing attention in those three ways. The session menu and the marked-session
menu offer an hour, until the next 06:00 on the device that sets it, or until you unmute. Questions and requests for
approval still draw attention.

## Things you should know

- The helm shares mutes across clients, carries them through Restart and Replace, and ends timed mutes with or without
  an open window. An unmute counts the session's current work as seen. It keeps its pre-mute place in the activity order
  until the next observed work start. A connected waiting session moves up for the wait.
- The slashed bell replaces the status dot, except on a connected waiting row, where it sits beside the title and the
  red dot stays. Click the bell to end the mute. Ended rows keep their status and name their mute in hover help and
  accessible text. The open session's header and details show the usual status with muted activity treated as seen.
- When a single update both ends a mute and shows an idle session whose last observed running state was muted, it stays
  quiet. A subsequent turn observed running without a mute can sound normally.
- The quick switcher follows the activity order served by the helm. The shared submenu and the marked menu are ready for
  session groups to reuse.

## Open questions and possible follow-ups

None for this round.

## Pull requests

- Specification and menu controls. [#4104](https://github.com/scode/farhelm/pull/4104/changes)
- Shared mute state, expiry, activity order and which work counts as seen.
  [#4106](https://github.com/scode/farhelm/pull/4106/changes)
- Shared submenu behavior and support for variable marked-menu actions.
  [#4107](https://github.com/scode/farhelm/pull/4107/changes)
- Desktop controls, sound behavior, screenshot references and TODO removal.
  [#4108](https://github.com/scode/farhelm/pull/4108/changes)

## Checks

- Ran `cargo nextest run -p farhelm-gui --lib` on the final desktop code with the filter below. All 657 passed, with no
  runtime skips. Recorder `509fe8c7-9a7e-48a9-a96d-5625d8bc81b9`. The narrow retry first passed the eight affected menu
  and fixture tests plus the compact row, marked mute and coalesced sound cases. Recorder
  `361c82a6-5112-4934-916e-fdf6c19232b6`.

  ```text
  test(/^session_list::/) | test(/^session_pane::details/) | test(/^operations/) | test(/^sounds::/) | test(/^local_time::/) | test(/^layers/) | test(/^components::submenu::/) | test(/^assets::/)
  ```

- Ran `cargo clippy --locked -p farhelm-gui --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `dprint check`
  on the edited Markdown, `python3 releasing/check-changelog.py format`, the Rust test sleep checker, and
  `git diff --check`. All passed.
- Rendered all 258 Linux reference screenshots with `python3 scripts/check-gui-goldens.py --update`. The light and dark
  session menu were the only changed files. Inspected and published both, then verified that all 258 published PNGs
  matched their captures. The capture wrote its completion marker before the command session timed out. Its recorder
  entry `6535ffcc-987c-4fab-bbe6-dbc5d16d97ce` has no recorded exit. The independent comparison with
  `python3 scripts/check-gui-goldens.py` then passed all 258 pixel comparisons. Recorder
  `333bda6c-8e0c-4b23-8fae-5fa29e15a267`.
- Reused 57 passing helm cases from `c0e7b3f9-4a9a-4fff-9f14-9d951fd58305` on the code pushed as `de9a18f1527e`. They
  cover mute expiry without a client, stored and shared state, ordering, Replace, Delete and the HTTP contract. The
  filter was
  `test(mute) | test(viewer_state) | test(pin) | test(mark_seen) | test(the_other_orders_lead) | test(http_contract_tests::)`.
  Also reused Clippy for helm and protocol all targets on that code, and 97 passing submenu and marked-menu cases from
  `062e6047-9b70-4df9-9f72-1144bcafff41` on the code pushed as `53558799aad7`. Both rebased patches were identical. The
  landed product changes affect launcher models and saved-template application.
- Skipped the full workspace and desktop smoke batteries because the targeted helm and GPUI cases exercise the changed
  paths. The Linux reference set is the only platform in the golden manifest. Validation ran on Linux.

## Review gate

All four drafts passed their independent reviews after fixes. The reviews checked that the right work counts as seen
when a mute ends, that clicks reach a child menu, that a marked session whose mute changes under an open menu is
rechecked, that the waiting bell sits beside the title, and that focus in a short menu reveals the highlighted choice.
The sound review prompted the rule for a single update that shows both a finished turn and an ended mute. The desktop
shares one duration menu, reads the helm's activity order, and uses its existing clock and sound paths.

### Landing

#### What landed with it

Since the stack was based, main gained only notes: a FLAKES.md entry with its TODO.md and known-flakes lines (#4097), a
TODO.md entry about quick switcher screenshots (#4102), and review-queue items (#4109). None of them touches the helm's
database, the session list, menus or sounds, and none interacted with this plan. The rebase applied cleanly. The helm
schema stays one step ahead of main, at 86.

This landing was reviewed with extra skepticism at your request, both by me and by an independent reviewer. Neither
found the work over-built for what the plan asked. The plan itself calls for:

- a helm table and an expiry timer;
- a shared submenu;
- a marked-rows menu that can list a changing set of actions.

The code is in proportion to that. The edge cases both reviews traced all hold:

- Delete, Replace or a new mute racing a timed expiry;
- an unreachable host;
- a muted session that starts waiting;
- marked rows that change while the Mute submenu is open.

#### Fixes made while landing

I ran the app and took screenshots of the row menu, the Mute submenu, and muted running, idle, waiting and ended
sessions, before and after these fixes:

- **The muted bell was too small to read.** It was drawn at the status dot's 8 pixels. At normal scale that is about as
  big as the dot itself, and the slashed outline blurs into a smudge. It is now 10 pixels, the size of the ended marks
  that share its slot, and centred where the dot was, so nothing else in the row moves. The two tests that pinned the
  bell to the dot's exact bounds now check that it keeps the dot's position.
- **The menus say what muting does and when it ends.**
  - The row menu's Mute line now reads "Stop routine work turning it blue or moving it up" instead of "Choose how long
    to mute this session".
  - Each duration choice now says when it would end: "Ends 18:40", "Ends tomorrow 06:00", "Ends when you unmute it".
    Before, the tooltips only repeated the labels. This also makes "Until tomorrow morning" honest when chosen before
    06:00, since it then ends the same morning.
  - The marked-rows menu says "Mute the marked sessions that are not muted", matching Pin's wording.
  - The changed row menu reference screenshots (light and dark) are republished.
- **A helm test fixture missed the new table.** The test that rebuilds a very old helm database from today's schema must
  drop every newer table. It did not drop the mute table, so two checkout configuration tests failed. The executor's
  test selection did not include them. The fixture now drops it, as its own comment asks of every new table.
- **Smaller fixes.**
  - The helm's HTTP contract test no longer sets the helm-only sort field, which is never sent.
  - A comment the store refactor dropped (why a cached session whose payload names another session is refused) is back.

#### Things you should know that the report does not say

- **The Mute submenu opens on click, Enter or Right only, not on hover**, as most desktop submenus do. While it is open,
  its parent item loses the highlight. The session groups plan reuses this component for four submenus, so whether it
  should open on hover is worth deciding before that plan runs.
- **The row menu no longer fits at the app's minimum window size.** With Mute added it scrolls, and Delete sits below
  the fold in the reference screenshot.
- **"Order hold" now means two things.** SPEC.md uses it for the pause in reordering while the pointer is over the list.
  The code now also uses it for the pre-mute place a muted session keeps. Renaming the mute one would avoid confusion;
  it touches several crates.
- **The helm's frozen sort key rides on the session row type that the desktop app also reads**, marked never-sent. It
  works, but it puts a helm-only sort input into the shared protocol type. Passing it beside the rows into the sort
  would keep the contract clean.
- **Mute waits on the same lock that tunnel opening holds while it connects**, so a Mute can wait behind a slow tunnel.
  The lock stops a late Mute from bringing back a row Delete just removed; pins live with the same race without it.
- **No test covers a mute ending through the timer while the helm runs.** The only expiry test starts the helm after the
  deadline has passed.

#### Checks

The stack changes the helm's database and ordering, the shared protocol, and the desktop app's session list, menus and
sounds. The landing therefore ran:

- the lints;
- the whole test suites of the GUI crates, the helm and the protocol;
- the app's end-to-end smoke;
- the screenshot goldens;
- a hands-on run of the app for screenshots.

The builds ran locally. The test suites and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the stack rebased onto main, before the landing fixes:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings` and `python3 -B scripts/check-gui-lints.py`: passed.
- `scripts/check-foundation-paths.py` (from the base revision), for each of the four PRs against the one below it:
  passed.
- `python3 scripts/check-gui-goldens.py`, locally: passed (`89d963eb-b416-4a14-8497-1f35ff0af00c`).

On the fixed app code:

- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py --no-build`), on a boat: 49 steps passed, with the
  clipboard paste step skipped as always on Linux (`5a502007-2678-446a-b799-c210b0d851e0`).
- `cargo nextest run -p farhelm-gui -p farhelm-grid-model -p farhelm-input-mapping -p farhelm-helm -p farhelm-proto`
  from a locally built archive, split in three across boats:
  - 1758 passed;
  - 1692 passed and 1 failed;
  - 1716 passed and 3 failed.
  - Runs: `c52b1c5d-0025-4dc9-88d1-e143bee4dd43`, `94b6a3db-e24a-4557-aaec-8b43c686b9b7`,
    `8eaabe33-1891-4333-a3f7-17754f137e39`.
- **The failures:**
  - Two GUI tests pinned the bell to the dot's exact size, which the size fix changed.
  - Two helm checkout configuration tests failed on the old-database fixture described above.
  - Both kinds are fixed.
- **Runtime skips:** two helm tests skipped at runtime because the archive does not carry a full `farhelm` binary. They
  are provisioning over the local transport and the uninstall stop. Neither touches mute; that coverage is missing from
  this run.

After the test fixes, which changed only tests and the old-database fixture, on the final tree, locally:

- `cargo fmt --all -- --check`, `dprint check` and
  `cargo clippy -p farhelm-gui -p farhelm-helm --all-targets -- -D warnings`: passed.
- `cargo nextest run -p farhelm-gui -p farhelm-helm --lib` for the session list's menu and compact tests and the helm's
  checkout configuration tests: 74 passed (`321b96f9-233d-467a-8522-685c7ce90f62`).
- `python3 scripts/check-gui-goldens.py` against the republished references: passed
  (`fbf92ff1-ed8d-420e-97f8-fda112252bd1`).

The boat suite and smoke runs were reused for the final commit, since only tests changed after them.

Skipped:

- The rest of the Rust workspace and the desktop process's tests: the change stays in the helm, the protocol and the GUI
  crates.
- The Linux app lifecycle: startup and quit are untouched.
- The Mac-only checks.
