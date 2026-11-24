# Busy Save button can be disabled mid-save and external pushes can overwrite fields while a save is in flight

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

While a custom-agent save is in flight, a pushed catalog update can disable the busy Save button (causing it to drop
keyboard focus) or overwrite the fields being saved with a subsequent external update.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/custom_agents.rs:301`.

At crates/gui/farhelm-gui/src/settings/custom_agents.rs:581-582, the Save button sets
.disabled(!this.dirty(cx)).busy(busy). Meanwhile, Editor::saw_saved (custom_agents.rs:301-310) runs from
sync_custom_agents with no check for self.busy().

1. If a pushed Collection::CustomAgents update arrives before Outcome::Done settles (e.g., flushed right before the
   reply by ui_channel::finish_command, or written by another client with matching values) and a render frame runs,
   saw_saved updates self.stored = Some(saved), making this.dirty(cx) become false while self.saving is still true. Save
   then becomes .disabled(true) while busy, violating SPEC_impl.md, 'Drafts' ('While a save is in flight its Save button
   is busy rather than disabled: it keeps focus and its place in Tab's order and ignores presses, as the old app's Save
   did, because a disabled button gives up focus'; compare settings/recipes/editor.rs:745-746).
2. Once stored matches the in-flight draft (making dirty false while self.saving is still true), a second external push
   before the reply settles makes clean == true in saw_saved (line 302) and calls self.restore(window, cx) (line 306),
   overwriting the text fields mid-save. How to verify: inspect custom_agents.rs:301-310 and 571-585. Fix: set
   .disabled((!this.dirty(cx) && !this.saving) || this.deleting) on custom-agent-save, and avoid calling restore() while
   self.busy() is true.

Reviewer metadata (g50_custom_agents_gui_settings_a/f9): suggested bucket: other (keyboard focus drop during save and
narrow concurrent-push draft overwrite).; confidence: confirmed (established via source and contract analysis; not
executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g50_custom_agents_gui_settings_a/f9`.
