# Multiple dirty custom-agent editors collapse under one 'Custom agent' label in Settings close prompt and jump to the wrong section

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

When two or more custom-agent pages have unsaved changes, the 'Save changes before closing?' prompt lists only a single
'Custom agent' and jumps Settings to whichever custom-agent draft sorts first rather than staying on the page the user
was viewing.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/editing.rs:141`.

At crates/gui/farhelm-gui/src/settings/editing.rs:141, Edit::name() returns the static string "Custom agent" for every
Edit::CustomAgent(section, _). In SettingsDialog::ask_to_close (editing.rs:585-593), affected form names are
deduplicated by edit.name(), and when names.len() == 1, Settings auto-switches to edits[0].section() via
reveal_save_failure(edits[0].section(), window, cx). Because self.custom_agents is a BTreeMap<Option<String>, _>
(editing.rs:271-276), if both AddCustomAgent (None) and CustomAgent("alpha") (Some("alpha")) — or two distinct
CustomAgent pages — are dirty, names deduplicates to ["Custom agent"] (len == 1). The close prompt hides that multiple
custom-agent forms are unsaved (SPEC.md: 'asks once with the affected forms named') and switches Settings away from the
user's current page to edits[0].section() (e.g. AddCustomAgent). How to verify: edit a field on Add custom agent and a
field on an existing custom agent, stay on the existing agent's page, and press Escape / Close. Fix: check distinct
sections (or edits.len() == 1) before auto-revealing edits[0].section(), and name distinct custom-agent drafts in the
close prompt (escaping host-supplied display names through HostSentence if shown).

Reviewer metadata (g50_custom_agents_gui_settings_a/f8): suggested bucket: other (confusing close prompt and unexpected
section switch when multiple custom-agent pages are dirty; saves and discards still cover all dirty drafts).;
confidence: confirmed (established via source and contract analysis; not executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g50_custom_agents_gui_settings_a/f8`.
