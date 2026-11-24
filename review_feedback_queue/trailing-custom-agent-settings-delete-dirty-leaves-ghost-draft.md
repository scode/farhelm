# Deleting a custom agent with unsaved edits leaves a ghost draft that Close->Save re-registers

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

If a user makes any unsaved edit to a registered custom agent and then deletes it, the deleted agent remains in the
Settings sidebar as an unsaved draft instead of being removed; choosing Save when closing Settings then silently
re-registers the agent they just deleted.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/custom_agents.rs:131`.

At crates/gui/farhelm-gui/src/settings/custom_agents.rs:413-427, Editor::delete's completion callback resets
editor.deleting = false and editor.confirm_delete = false on Outcome::Done(_) without removing the editor from
parent.custom_agents or clearing its dirty draft. Meanwhile, SettingsDialog observes HelmStore only via cx.notify()
(crates/gui/farhelm-gui/src/settings.rs:239) and calls sync_custom_agents() only inside SettingsDialog::render()
(crates/gui/farhelm-gui/src/settings.rs:1740). On the helm side, ui_channel::finish_command
(crates/farhelm-helm/src/ui_channel.rs:1082-1100) sends ServerMessage::Changes (removing the deleted entry from
Collection::CustomAgents) immediately followed by ServerMessage::Reply(204). In GPUI, both messages are processed on the
foreground executor before the next render frame, so editor.deleting is already false when sync_custom_agents() runs. At
custom_agents.rs:128-136: let keep = !draft.deleting && (draft.saving || draft.dirty(cx)); Because draft.deleting is now
false and draft.dirty(cx) is still true, keep evaluates to true. sync_custom_agents retains the editor in
self.custom_agents, sets editor.removed = true (line 148), and navigates to AddCustomAgent (lines 132, 154-158), making
the deletion look complete. However, the deleted agent stays in custom_sections() (lines 71-76) with an unsaved dot and
stays dirty in SettingsDialog::edits (crates/gui/farhelm-gui/src/settings/editing.rs:271-276). When the user closes
Settings, 'Save changes before closing?' appears for 'Custom agent'; clicking Save invokes Editor::save, which sees
self.removed == true (custom_agents.rs:326) and sends POST /api/custom-agents, silently re-registering the deleted
agent. Note also that Editor::delete silently clears deleting/confirm_delete on Outcome::Accepted(_) (lines 420-423)
without reporting an error. The existing test creating_and_deleting_a_custom_agent_follows_the_catalog
(settings/tests.rs:358-371) missed this because it deletes a clean registration (dirty == false). How to verify: in a
GPUI test, edit a field on a registered custom agent without saving, click Delete then Confirm Delete, push
CollectionChanges removing the agent and reply 204 before settle(cx), and observe that the sidebar row and
unsaved-changes state remain. Fix: on Outcome::Done(_) in Editor::delete, explicitly remove the editor from
parent.custom_agents (and navigate away if shown) rather than relying on render-time sync observing draft.deleting ==
true, and treat Outcome::Accepted(_) as an error.

Reviewer metadata (g50_custom_agents_gui_settings_a/l1): suggested bucket: high (moderate common workflow — editing then
deleting a custom agent — leaves a phantom sidebar entry and can silently re-register a deleted agent on Settings
Close->Save; does not lose user data, credentials, or running processes).; confidence: confirmed (established via source
and contract analysis; not executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g50_custom_agents_gui_settings_a/l1`.
