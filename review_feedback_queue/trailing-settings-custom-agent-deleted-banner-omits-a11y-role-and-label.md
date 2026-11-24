# External-deletion banner in custom-agent editor omits accessibility Role and aria_label

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

When a custom agent is deleted externally while the user has unsaved edits, the 'This registration was deleted. Register
it again to keep these edits.' banner is omitted from the accessibility tree so assistive technology never exposes or
announces it.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/custom_agents.rs:686`.

At crates/gui/farhelm-gui/src/settings/custom_agents.rs:685-687, Editor::render draws the deleted-registration notice as
a bare div with no element id, Role, or aria_label: .children(self.removed.then(|| { div().child("This registration was
deleted. Register it again to keep these edits.") })) As documented at crates/gui/farhelm-gui/src/host_text.rs:580-581
('GPUI puts an element in the accessibility tree (and the bridge's record) only when it declares a role; a div's label
alone is dropped') and applied in settings/choices.rs:641-651 (choices::outcome_line), status/alert lines in Settings
must declare an id, Role::Status (or Role::Alert), and aria_label to reach assistive technology. How to verify: inspect
custom_agents.rs:685-687 alongside host_text.rs:580-586 and settings/choices.rs:641-651. Fix: render the banner through
choices::outcome_line (or with .id(...).role(Role::Status).aria_label(...)).

Reviewer metadata (g50_custom_agents_gui_settings_a/f16): suggested bucket: other (accessibility omission on an
edge-case status banner).; confidence: confirmed (established via source and contract analysis; not executed).; possible
cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g50_custom_agents_gui_settings_a/f16`.
