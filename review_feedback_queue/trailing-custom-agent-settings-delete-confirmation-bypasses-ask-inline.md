# Custom-agent Delete confirmation bypasses prompt::ask_inline, breaking Cancel focus, Escape handling, and drawn-frame binding

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

Confirming Delete on a custom agent uses raw footer buttons instead of Farhelm's prompt control, so keyboard focus does
not move to Cancel, pressing Escape closes Settings instead of cancelling the delete confirmation, start errors
overwrite the confirmation question text, and the Delete handler reads live editor state instead of a frozen
confirmation target.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/custom_agents.rs:509`.

SPEC_impl.md, 'Questions: the prompt', establishes that the prompt module is the only way to ask a confirmation
question, freezing targets/consequences/preconditions when opened, pre-wiring answer buttons to that frozen state with
when_shown protection, moving focus to the starting answer (Cancel for destructive actions) and returning focus on
close, and consuming Escape as Cancel inside inline questions so it does not reach the owner dialog. Compare
settings/recipes/editor/leaving.rs:226-288, which uses prompt::ask_inline(Question::new(...).starts_on_cancel(), ...).
In crates/gui/farhelm-gui/src/settings/custom_agents.rs:500-595, Editor::footer instead branches on self.confirm_delete
and renders raw Button elements (custom-agent-keep and custom-agent-confirm-delete):

1. Clicking custom-agent-delete (lines 551-556) only sets confirm_delete = true; it removes the focused Delete button
   without focusing Cancel, and clicking Cancel does not restore focus.
2. Pressing Escape while the Delete confirmation is open bubbles to SettingsDialog's on_dismiss (settings.rs:263-270),
   closing Settings (if clean) or opening the 'Save changes before closing?' modal (if dirty) instead of cancelling the
   Delete question.
3. custom-agent-confirm-delete's click handler (lines 535-537) calls Editor::delete, which reads live self.stored
   (line 392) rather than a frozen target captured when the confirmation opened and gated by prompt's when_shown, and
   the question text ('Delete this custom agent?...', line 591) does not name which registration is being deleted.
4. If Editor::delete fails to start (line 433), self.error is set while self.confirm_delete stays true; notice (lines
   588-593) renders this.error.as_ref().map(...).or_else(|| confirming.then(...)), replacing the confirmation question
   text with the error while Cancel and Delete buttons remain in the footer. How to verify: inspect
   custom_agents.rs:391-435 and 500-595 against settings/recipes/editor/leaving.rs:226-288 and SPEC_impl.md, 'Questions:
   the prompt'. Fix: replace confirm_delete: bool and the raw footer confirmation buttons with
   Option<prompt::Inline<String>> created via prompt::ask_inline with .starts_on_cancel(), naming the frozen
   registration in the question and passing the frozen ID to Editor::delete.

Reviewer metadata (g50_custom_agents_gui_settings_a/l5): suggested bucket: high (Escape closes Settings instead of
cancelling a destructive confirmation, keyboard focus on Cancel is broken, and the prompt/drawn-frame confirmation
contract is bypassed. Uncertainty about highest: each Editor entity is keyed by an immutable registration ID so
self.stored.id cannot swap to a different agent's ID, and Escape closes rather than confirming deletion, so no
established route deletes an unintended target; however, external edits can change the agent's name/command while the
unnamed confirmation is open).; confidence: confirmed (established via source and contract analysis; not executed).;
possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g50_custom_agents_gui_settings_a/l5`.
