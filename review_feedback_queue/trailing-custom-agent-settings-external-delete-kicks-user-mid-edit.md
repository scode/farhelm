# External deletion of a custom agent kicks the user off the page mid-edit and hides in-flight save errors

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

If a custom agent is deleted from the CLI or another client while the user is editing or saving it in Settings, Farhelm
abruptly switches them to the Add custom agent page even though it keeps their draft in the sidebar, interrupting their
typing and hiding both the 'deleted — register again' banner and any 404 save failure.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/custom_agents.rs:132`.

At crates/gui/farhelm-gui/src/settings/custom_agents.rs:128-158, when collections.get::<CustomAgent>(id) is None while
an editor is kept for unsaved or in-flight edits (keep == true), line 132 sets: selected_deleted |= selected.as_ref() ==
Some(id) && (!keep || !draft.removed); On the first observation of the external deletion (!draft.removed is true),
selected_deleted becomes true even when keep is true. Lines 154-158 then call self.show(Section::AddCustomAgent, window,
cx), switching the user away from the editor they are actively typing in — even though the editor is retained with
editor.removed = true (line 148), which has a dedicated banner ('This registration was deleted. Register it again to
keep these edits.', line 686) and 'Register again' button (line 574). This violates SPEC.md, 'Keyboard focus' -> 'Things
changing underneath' ('A refresh never changes what the user is typing. Text, caret and focus stay as they are, and only
what the user is not editing updates'). Furthermore, when a PUT is in flight and subsequently fails with 404
(custom_agents.rs:368-371), editor.error is stored on the now-hidden page and never shown unless the user notices the
retained sidebar row and clicks back, violating SPEC.md, 'Errors and diagnostics' ('Every failed operation surfaces a
concrete, actionable error in the client'). Note that settings/tests.rs:326 currently asserts this switch to Add custom
agent during an in-flight PUT. How to verify: inspect custom_agents.rs:131-158 and settings/tests.rs:320-340. Fix:
change line 132 to `selected_deleted |= selected.as_ref() == Some(id) && !keep;` so only discarded (clean or explicitly
deleted) editors navigate away to Add custom agent, while retained dirty/saving editors stay on screen showing the
removed banner and any PUT refusal.

Reviewer metadata (g50_custom_agents_gui_settings_a/l2): suggested bucket: high (yanks focus/view away mid-typing on a
background push and hides save failures and draft recovery; drafts remain preserved in the sidebar so user work is not
lost).; confidence: confirmed (established via source and contract analysis; not executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g50_custom_agents_gui_settings_a/l2`.
