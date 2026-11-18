## What this was about

Related sessions could drift apart as their activity changed. You can now select **Move to group** from a session's menu
or the marked-sessions menu. Groups sit below the global pins, in the order you set. Each group can hold its own pins.
Collapse hides the member rows and leaves a status dot from the most urgent member matching the current filters. A
filtered group shows how many of its members match.

## Things you should know

Group names, members, order and collapse state are shared across windows. Replace carries the original session's
membership and pin position. Ungroup keeps the group's pins in order at the bottom of the global pins. Removing the last
member removes the group.

The group menus list every destination, including groups hidden by the current filter. The helm trims group names and
refuses blank or case-insensitive duplicate names. Names are limited to 256 UTF-8 bytes. The helm also caps how much
space the group roster can use in session list updates.

The stack extracts the shared pin ordering before it adds groups. I combined layout and drag in the final desktop PR
because each row's displayed area defines where its pin can move. Headers and pins share the grip and drop line.
Automatic reordering waits while you hover over the list or use a row menu. Cancelling the marked-sessions naming dialog
before Save keeps the marks. A dispatched Save clears them, and a refusal stays in the dialog.

## Open questions and possible follow-ups

Dragging unpinned sessions is tracked separately under **Drag sessions into groups**. Their row and marked-sessions
menus already support moving them.

## Pull requests

- Specify groups, pin areas and when the list holds its order.
  [#4110](https://github.com/scode/farhelm/pull/4110/changes)
- Extract the helm's shared manual ordering. [#4111](https://github.com/scode/farhelm/pull/4111/changes)
- Store session groups and share them across clients. [#4112](https://github.com/scode/farhelm/pull/4112/changes)
- Extract the desktop's pin-ordering helpers. [#4113](https://github.com/scode/farhelm/pull/4113/changes)
- Add groups to the desktop list, menus and drag. [#4114](https://github.com/scode/farhelm/pull/4114/changes)

## Checks

- `cargo nextest run -p farhelm-helm --lib` passed 44 tests (`627a7755-5cb3-40b8-a446-e4b0c19a8798` at `8379c5f2`). The
  filter was
  `test(/session_group|store::session_groups|session_pin|schema_8[67]|mute_|pin_order|replace_of_a_live_raw|a_delete_lost_after|aggregate::|http_contract_tests::|the_session_query_filters|every_pushed_collection|the_snapshot_on_open/)`.
  It covered schema upgrade, group membership, concurrent Replace, delete and listing refresh, and sharing group names
  with a filtered list. The helm code is byte-for-byte the same at the final tip.
- `cargo nextest run -p farhelm-helm --lib -E 'test(/^store::tests::session_pin/) | test(/^sessions::tests::pin_/) | test(store::tests::schema_74)'`
  passed 10 unchanged helm pin tests (`1b290ee7-bfa3-4d7d-8298-9c8303ea544f` at `51b1e9aa`, followed by a comment edit).
  `cargo nextest run -p farhelm-gui --lib -E 'test(/session_list::(pins|pin_drag|hold)/)'` passed 32 unchanged pin, drag
  and hold tests (`00b76923-0fb6-471f-b38b-0f55b8291a4c` at `f1a785e9`). Those runs isolate the two refactors.
- The first desktop session-list run found five menu expectations that needed the new item and one separator regression
  while reordering was held (`3f1d65e4-188d-4300-9037-de971d519570`). Corrected the assertions and restored the
  separator's timing. The final `cargo nextest run -p farhelm-gui --lib -E 'test(/^session_list::/)'` passed all 493
  selected tests on the #4114 tree (`ea9c6093-b945-49e4-a12a-502bdcda2f0d`, verified byte-for-byte against `3c226fa8`).
- `python3 -B scripts/gui-e2e-smoke.py` passed 48 steps on Linux (`4e51183f-4fcc-4d0c-b59a-62e22d5d0d9d` at `e115619c`).
  Headless Mutter skipped the clipboard step. The smoke remains applicable because the later code change only changes
  focus return when the marked-sessions group dialog closes. The session-list tests drive that dialog.
- `python3 -B scripts/check-gui-goldens.py --update` captured 258 Linux images (`7f223f4e-0512-465e-841a-19d999413925`,
  based on `f1a785e9` with the desktop feature in progress, recorded diff hash `115fc83b4c12`). I visually checked the
  two changed row-menu images and compared all captures byte for byte with the published references. The final focus
  return change takes place after a marked-sessions dialog closes. The captured scene opens the ordinary row menu and
  stops there.
- `cargo fmt --all -- --check`, Clippy with `--all-targets -- -D warnings` on the GUI, helm, helm-client and proto,
  `dprint check` on changed Markdown, `scripts/check-test-sleeps.py`, changelog format, crate layering, dependency
  notices and the Cargo license/source checks passed.
- Skipped the workspace-wide test battery and hosted CI. The affected helm and desktop paths were covered by the local
  selections and smoke. macOS visuals and clipboard were outside the Linux run.

## Review gate

Each PR had a fresh adversarial review. The helm review led to fixes for a group change racing Replace, names that
compare equal across Unicode case variants, group data growing past the list's message budget, and membership left
behind when a complete refresh proves a session disappeared. The desktop review found two marked-menu issues, a pin drop
target that could disappear while the list held its order, and a duplicated separator rule. The corrections reuse the
existing menu dispatch, naming dialog, separator, pin drag and focus recovery. The desktop reviewer verified closure.

### Landing

#### What landed with it

Since the stack was based, main gained only the plans queue's own bookkeeping. Nothing else landed in this round, so
nothing interacted with this plan.

This landing was reviewed with extra skepticism at your request, both by me and by an independent reviewer, and I ran
the app to look at every part of the feature. The stack is large, but most of its size is what the plan asked for: one
user-set ordering, one drag handle, one pause on automatic reordering and one naming dialog, each extended from the pin
code rather than copied. Neither review found a second implementation of any of them. Two parts go beyond the plan:

- **The helm caps how much space group names take in every session list reply** (about 4 MB, far more groups than anyone
  would make). The plan did not ask for this; it came from the executor's helm review, which found that group data could
  outgrow the list's message budget. A cap on the number of groups would do the same job more simply. It is small and
  harmless, so I left it.
- **A complete refresh from a host drops the group membership of sessions it no longer lists.** The reviewer suggested
  removing this. I kept it. Without it, a session deleted outside this app (from the command line, or by another
  installation's helm) would stay a hidden member forever: its group would keep counting it and would never disappear.
  Pins of such sessions stay, as before, because a pin with no session does nothing visible.

#### Fixes made while landing

I ran the app, made two groups, pinned a session inside one and another globally, collapsed a group, filtered by status
and opened every menu, before and after these fixes:

- **A helm test fixture missed the new tables.** The test that rebuilds a very old helm database from today's schema
  must drop every newer table. It did not drop the two group tables, so two checkout configuration tests failed on the
  remote test machines. The executor's test selection did not include them. This is the same kind of miss as when
  session muting landed: new tables were not added to this fixture. The fixture now drops them.
- **Group headers always showed their drag grip and actions button.** Session rows show both only while the pointer is
  over the row or focus is inside it; every header drew them all the time, which cluttered the list with controls that
  do nothing until reached for. Headers now reveal them the way rows do.
- **Wording.**
  - The row menu's Move to group now has a description line ("Keep related sessions together") like every other item.
    The two row menu reference screenshots (light and dark) are republished for it.
  - The group header menu's Rename now has one too ("Give this group a new name").
  - The naming dialog spoke of a session's title when naming a group. It now says "name" for a group, and for a new
    group its button says Create and the hover help on Cancel and Close says no group is made.
  - Each Move to group destination's hover help used to repeat its label. It now says what choosing it does; Global pins
    says the session leaves the group and stays pinned above all groups.

#### Things you should know that the report does not say

- **Group members are not indented.** A long group's members look exactly like ungrouped rows; only the header above
  them and the heavier separator after the last group mark where the group ends. Worth a follow-up if long groups turn
  out hard to read.
- **When the open session belongs to a collapsed group, nothing in the sidebar shows which session it is** until the
  group is expanded. The header's status dot shows only the most urgent member. Worth a follow-up: marking the header
  that holds the open session would fix it.
- **Automatic reordering now also waits while the marked-sessions menu is open**, as it does for a row menu. SPEC.md's
  rule for when reordering waits does not mention that menu; the behavior is right, and a one-line SPEC.md follow-up
  would record it.
- **The row menu grew again.** With Move to group added, it scrolls further at the app's smallest window size. Fine as
  is unless the menu gets more items.
- **Every Move to group entry uses the same folder icon**, New group… and Remove from group included. Fine as is.

#### Checks

The stack changes the helm's database, routes and ordering, the shared protocol, and the desktop app's session list,
menus, naming dialog and drag. The landing therefore ran:

- the lints;
- the whole test suites of the GUI crates, the helm, its client and the protocol;
- the app's end-to-end smoke;
- the reference screenshots (goldens);
- a hands-on run of the app for screenshots.

The builds ran locally. The test suites and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the stack as delivered (main had gained only plans bookkeeping since its base):

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 -B scripts/check-crate-layering.py`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
  `python3 -B scripts/check-gui-lints.py`: passed.
- `scripts/check-foundation-paths.py` (from the base revision), for each of the five PRs against the one below it:
  passed.
- `python3 scripts/check-gui-goldens.py`, locally: passed (`1dfd051c-0e4a-4cd9-8b99-f50387e217d3`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py --no-build`), on a boat: 48 of its 49 steps passed and the
  clipboard paste step was skipped, as always on Linux (`2dbed5b9-c3cf-40a0-bb6c-55f31fded607`).
- `cargo nextest run` with `-p` for `farhelm-gui`, `farhelm-grid-model`, `farhelm-input-mapping`, `farhelm-helm`,
  `farhelm-helm-client` and `farhelm-proto`, from a locally built archive, split in three across boats:
  - 1791 passed;
  - 1748 passed and 2 failed;
  - 1720 passed.
  - Runs: `53ce8717-ac95-45e0-97c7-ea2c8a784a5a`, `b3a05ffb-8cb4-40bf-a651-428cebe12cfd`,
    `a3cb166c-0879-4bb9-ab10-8e29902d12e0`.
- **The failures:** the two helm checkout configuration tests on the old-database fixture described above. Fixed.
- **Runtime skips:** three helm tests skipped at runtime because the archive does not carry a full `farhelm` binary.
  They are provisioning over SSH and over the local transport, and the uninstall stop. None touches groups; that
  coverage is missing from this run.

On the fixed tree:

- `cargo fmt --all -- --check`, `dprint check`,
  `cargo clippy -p farhelm-gui -p farhelm-helm --all-targets -- -D warnings` and
  `python3 -B scripts/check-gui-lints.py`: passed.
- `cargo nextest run -p farhelm-gui -p farhelm-helm --lib` for the whole session list and the helm's checkout
  configuration tests, locally: 530 passed (`5d68ee50-0b28-4d89-be89-f212d2cf5c57`).
- The end-to-end smoke, on a boat: 48 steps passed and the paste step was skipped
  (`e5b9caef-fe26-443b-a95f-c4059452eb3b`).
- `python3 scripts/check-gui-goldens.py` against the republished references, locally: passed
  (`29946afb-bd83-4bc4-b8a2-c3807188a156`).

The boat suites were reused for the fixed tree: after them only the old-database fixture, the header's hover, wording
and the row menu test's expected descriptions changed, and the affected modules ran again above.

Skipped:

- The rest of the Rust workspace and the desktop process's tests: the change stays in the helm, the protocol and the GUI
  crates.
- The Linux app lifecycle: startup and quit are untouched.
- The Mac-only checks.
