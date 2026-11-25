# Template editor's Save and Duplicate can act on a template not yet shown

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

In the Templates dialog, a click on Save or Duplicate that lands just after a click on another template's row (before
the screen redraws) acts on the newly opened template the user has not seen, not on the one shown. When both are older
templates that Farhelm updates on opening, Save can write those automatic updates (the inferred launcher-tab switch,
dropping the retired workspace-trust field) into a template the user never saw or saved, which changes how that template
applies afterwards. Duplicate opens a copy of the wrong template.

## Details

Pre-existing: the Save and Duplicate behavior predates the reviewed range, but 4095a64022 (the fix that bound Delete to
the template its frame showed) fixed the same defect class in the same footer and left these two buttons unbound.
Locations: `crates/gui/farhelm-gui/src/templates/editor.rs:1286-1290` (Save's `on_click` calls `dialog.save_editor` with
no identity), `editor.rs:1292-1305` (Duplicate's `on_click` calls `dialog.duplicate` with no identity),
`dialog.rs:475-484` (`save_editor` saves `self.editor`, whatever it is now), `dialog.rs:466-469` and `:642-668`
(`duplicate` -> `leave(Departure::Duplicate)` -> `depart` duplicates `self.editor`).

The commit's own regression test (`an_old_delete_footer_cannot_delete_the_new_editor` in `leave_tests.rs`) establishes
that a click can reach a frame's footer after a row click has switched editors (`test_support::click_before_redraw` on a
row, then on the old footer). 4095a64022 fixed Delete by capturing `self.previous` at render and checking
`editor.previous() == Some(name)` in `delete_editor` (`dialog.rs:487-504`). Save and Duplicate in the same footer
capture nothing: their closures call `dialog.save_editor(window, cx)` and `dialog.duplicate(window, cx)`, which act on
`self.editor` at handling time.

Reachability: the row switch happens without a leave question only when the current editor's
`needs_leave_confirmation()` is false (`editor.rs:412-419`). That includes a legacy kindless or trust-carrying template
whose Save is enabled solely by Farhelm's fill-ins (SPEC.md: "An inferred switch or removed legacy workspace-trust field
enables Save but never makes leaving ask"). A busy button ignores presses (`components/button.rs:1084`), and Save is
busy exactly when the drawn editor is not dirty (`editor.rs:1279`), so the stale press acts only when the shown template
X itself showed Save enabled. If the newly opened template Y is also such a legacy template, `Editor::save`
(`editor.rs:750-758`) passes its `!dirty()` guard and writes Y with the inferred switch stored and trust dropped.
SPEC.md says "Existing kindless templates keep applying as stored; opening one infers its switch in an unsaved draft,
and saving stores that switch", so after this stale save Y switches launcher tabs when applied, a change the user never
saw or chose. If Y is non-legacy and untouched, save refuses on `!dirty()` and nothing happens; if Y fails current save
rules, the status line shows Y's refusal while the user believes they saved X. Duplicate: a stale click opens an unsaved
copy of Y named Y-copy, harmless beyond confusion.

Verify by reading the three `on_click` closures in the editor footer (`editor.rs` around 1270-1330) next to the name
filter in `delete_editor`. Fix: capture the editor identity at render (the name field's FocusHandle, as 2cc03220c2 does
for leave answers via `is_open_editor`, or `previous`) in the Save and Duplicate closures, and ignore the click when the
dialog's current editor differs.

Bucket: other. The worst established consequence is an unrequested normalization write to a legacy template (its
inferred switch stored, its ignored trust field dropped); no user content is lost, and the write is visible in the list
afterwards. Not highest: the write stores only values Farhelm itself derived from the stored template, and the dropped
field is ignored on apply per SPEC.md.

Confidence: likely. Source supports every step; the premise that a real click can land on the previous frame's footer
after a row click is taken from the commit's own test and its triage outcome
(`template-delete-can-target-unshown-editor.md`, fixed), not observed here.

Possible cover: none known; SPEC.md's "The label the user saw decides" covers only Delete.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g20 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
