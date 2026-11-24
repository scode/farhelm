# Closing Settings with Save silently fails with no error when a custom agent's Delete confirmation is open

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

If a user edits a custom agent, clicks Delete to show the confirmation buttons, and then closes Settings and chooses
Save in the 'Save changes before closing?' dialog, saving silently fails without displaying any error and leaves
Settings stuck open on the delete confirmation (and leaves any remaining dirty Settings forms unsaved).

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/custom_agents.rs:317`.

At crates/gui/farhelm-gui/src/settings/custom_agents.rs:274-276, Editor::edit_state returns EditState::Dirty whenever
!self.deleting && self.dirty(cx), including when self.confirm_delete is true (and switching away from the custom-agent
page does not clear confirm_delete). When the user triggers Settings close via Escape or the header Close button (which
remains visible while confirming), SettingsDialog::ask_to_close (crates/gui/farhelm-gui/src/settings/editing.rs:572-646)
opens 'Save changes before closing?'. Choosing Save calls start_batch(None) -> dispatch_edit (editing.rs:425-434) ->
Edit::CustomAgent::save -> custom_agents::Editor::save (custom_agents.rs:316-319). Line 317 executes: if self.busy() ||
self.confirm_delete { return false; } It returns false without setting self.error and without clearing
self.confirm_delete. dispatch_edit treats !started as a save failure (editing.rs:432-434), aborts the batch (leaving any
subsequent dirty forms unsaved), and reveals the custom-agent page (editing.rs:467) with no error message and with the
Delete confirmation still replacing the footer. This violates SPEC.md, 'Errors and diagnostics' ('Every failed operation
surfaces a concrete, actionable error in the client'). Compare settings/recipes/editor.rs:523-526 and leaving.rs:218,
where save() does not refuse an open delete confirmation and leaving clears it. How to verify: edit a custom agent
field, click custom-agent-delete, press Escape or click Settings header close, choose Save in the close prompt, and
observe Settings stays open on the delete confirmation with no error and no PUT sent. Fix: clear any open delete
confirmation when Editor::save is invoked (and when switching sections or opening the Settings close prompt).

Reviewer metadata (g50_custom_agents_gui_settings_a/f6): suggested bucket: high (silently breaks Settings Close->Save
with no error and aborts saving remaining dirty forms; no user data is lost because drafts remain open).; confidence:
confirmed (established via source and contract analysis; not executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g50_custom_agents_gui_settings_a/f6`.
