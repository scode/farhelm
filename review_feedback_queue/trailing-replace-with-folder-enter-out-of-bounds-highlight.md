# Replace with folder Enter bypasses the presentation guard when the highlighted index goes out of bounds

Reviewed commit: c5cd636e03c3b6633853f46a8146c242a86fe383

## TLDR

In Replace with, if a highlighted folder dropdown item becomes out of range after the folder changes (for example, when
a template shortcut or delayed template batch updates the folder while focus stays in the filtered folder field),
pressing Enter bypasses the new presentation guard and immediately replaces—and deletes—the source session instead of
keeping the user safe.

## Details

**highest.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view/folder_list.rs:221`.

In `crates/gui/farhelm-gui/src/launcher/view/folder_list.rs:221-246`, `Launcher::folder_enter` checks
`self.folder_list.list.highlighted.is_none()` before checking `enter_can_replace` (lines 222-231), and
`follow_folder_list` (lines 175-182) likewise captures `list.list.highlighted.is_none()`. When `highlighted` is
`Some(index)` but `index >= self.folder_choices(cx).len()`, the `enter_can_replace` guard is skipped,
`self.folder_choices(cx).get(index)` returns `None` at line 239, and the `else` branch closes the dropdown and returns
`false` (lines 240-243), causing `launcher/view.rs:851-854` to immediately call `launcher.press(window, cx)` and submit
`ReplaceWith` (retiring/deleting the source session).

Established route in shipped code:

1. Open Replace with on a session whose host has >= 2 recent folders (e.g. `/proj/a`, `/proj/b`), focus the Folder
   field, type `proj` (setting `folder_list.filtering = true` in `folder_typed` at `folder_list.rs:192-198`), and press
   Down twice (`highlighted = Some(1)`).
2. Trigger a template batch that sets a different folder (e.g. via a template shortcut like Cmd+P/Cmd+1, which leaves
   focus in the folder field per SPEC.md, or an already-queued batch in `waiting_batches` applying asynchronously when
   the catalog read answers at `launcher/view.rs:508-512`).
3. `Launcher::apply_now` (`launcher/view.rs:1021-1066`) replays `draft.edit_folder(...)` (`templates/apply.rs`) and
   calls `sync_fields` -> `self.folder.set_text(...)` (`launcher/view.rs:1111,1126`), which emits no
   `TextInputEvent::Changed` (`components/text_input.rs:316-325`) and neither drops `folder_list.list.highlighted` nor
   clears `folder_list.filtering`.
4. `folder_choices(cx)` now filters against the new folder text and shrinks to `[FolderChoice::Home]` (len 1), while
   `highlighted` remains `Some(1)`. No row draws highlighted (`draw_folder_rows`, `folder_list.rs:328`),
   `enter_can_replace` still captured `false`, and pressing Enter (even before any frame is presented) skips lines
   222-231, hits `None` at line 239, returns `false`, and immediately invokes `launcher.press(window, cx)` to replace
   and delete the source session. (If `highlighted` was `Some(0)`, Enter instead silently picks `FolderChoice::Home`
   rather than the previously highlighted recent folder.)

Fix direction: Drop folder highlight (via `drop_folder_highlight()` / `folder_picked()`) whenever `draft.folder` changes
outside `folder_typed` (including in `apply_now` when folder text changes), and in `folder_enter` / `follow_folder_list`
treat an out-of-bounds `Some(index)` identically to a stale/missing highlight (never falling through to `return false` /
`launcher.press` in `ReplaceWith` without a presented no-highlight `ActionFrame`, and ideally storing folder highlight
by `FolderChoice` identity rather than raw `usize` index).

Reviewer metadata (g03-triage-gui-shortcuts-launcher-1/f1): suggested bucket: highest (could be in highest: confirmed
route submits Replace with and deletes the source session without presenting a no-highlight state or confirming
replacement); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as
`g03-triage-gui-shortcuts-launcher-1/f1`.
