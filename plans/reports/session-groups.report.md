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
