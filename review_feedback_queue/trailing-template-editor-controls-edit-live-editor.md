# Template editor field controls still edit whichever editor is open

Reviewed commit: 446755516cbe3ac46f8de7147aea0210a93ffb2e

## TLDR

In the Templates dialog, a click that lands on the old editor's controls just after another template was opened from the
list is applied to the newly opened template. The commit fixed this for Save and Duplicate, but every other editor
control (Replace name, + Add field and its items, each field's remove button, the launcher-kind switch, agent,
permissions and other radio/select fields, model suggestions, host, destination type, session name kind) still edits the
new template's draft. Nothing is saved by that click alone, but the second template now carries a change the user made
to the first one, for example an added field, a removed field, or a changed permission setting, and a later Save stores
it.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t12a-gui-panes-hosts` `f8` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept: the stray edit stays in a visible, unsaved draft and the leave question asks before
discarding. It would move up if a draft edit could be saved without the user seeing it. No coverage source covers it.

### Reviewer record

Location: `crates/gui/farhelm-gui/src/templates/editor.rs:1577`

At 446755516c (unchanged at the tip), `on_editor` and `with_editor`
(crates/gui/farhelm-gui/src/templates/editor.rs:1568-1589) run their action on `dialog.editor.as_mut()`, the live
editor, with no check that it is the instance whose frame drew the control. Callers: editor.rs:1079 (Replace name), 1186
(+ Add field toggle), 1212 (add-field items); crates/gui/farhelm-gui/src/templates/editor/rows.rs:196 (Switches launcher
to), 260 (field remove ✕), 436 (agent select), 479 and 509 (radio/select fields set through `set`, including
permissions), 571 (model suggestion), 661 (placement: runs on, recipe, container host), 725 (host), 759 (destination
type), 809 (session name kind). The commit message states the premise these share: "A click can reach the Templates
footer after a different editor has already opened", and its test `old_save_and_duplicate_cannot_act_on_the_new_editor`
(crates/gui/farhelm-gui/src/templates/leave_tests.rs) produces that state with `click_before_redraw` on a list row
followed by a press on the old frame's control. The commit added the instance check (`TemplatesDialog::is_open_editor`
through the name field's focus handle) only to Save and Duplicate; Delete already carried its own snapshot. The value a
stale handler applies is captured from the old editor's frame (for example the radio `value` or the field to add or
remove) and applied to the new draft, so the new template gets a field or value nobody chose for it. SPEC_impl.md's rule
is that a handler acts on what the render the user acted on showed ("This is the structural fix for the old GUI's
largest bug class: an action that ran on state the user was not shown"). How to verify: in leave_tests.rs's setup, open
one template, `click_before_redraw` on another template's row, then on the first editor's `templates-add-*` or
`templates-remove-*` button or a permissions radio, and assert the new editor's draft is unchanged. Fix: give
`on_editor`/`with_editor` the drawn editor's identity (the name field's focus handle, as the footer now captures it) and
make them do nothing when `is_open_editor` is false, so every editor control gets the same guard in one place.

Suggested bucket: other, because the stray edit stays in an unsaved, visible draft (the editor shows it and the leave
question asks before discarding), so harm needs the user to Save without noticing; it would move up if a draft edit
could persist or launch without a Save, which these handlers do not do.

Confidence: confirmed (source-established that every listed handler acts on the live editor); the timing premise (a
press reaching the old frame after a row switch) is the one the commit itself accepts and tests.

proposed_drop: none

possible_cover: none
