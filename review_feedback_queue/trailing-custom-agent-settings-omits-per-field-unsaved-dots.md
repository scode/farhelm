# Custom-agent editor fields never show per-field unsaved dots required by Settings SPEC

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

Editing Display name, ID, Command, or capability checkboxes on a custom-agent page marks the sidebar row and footer as
unsaved, but never shows the unsaved dot next to any edited field.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/custom_agents.rs:463`.

SPEC.md ('Session list', Settings dialog paragraph, line 2564) specifies: 'A dot marks each edited field and section.'
Compare settings/recipes/editor.rs:792-797, which passes per-field edited state to editing::caption. At
crates/gui/farhelm-gui/src/settings/custom_agents.rs:463, Editor::field unconditionally passes None as the edited
palette argument to choices::labelled(label, id, field, None), and Editor::capability (lines 484-485) omits the edited
dot as well. How to verify: inspect custom_agents.rs:449-498 and 644-673. Fix: compare each field against self.stored
(or empty/false for Add) and pass edited.then_some(theme::palette(cx)) to choices::labelled / editing::caption.

Reviewer metadata (g50_custom_agents_gui_settings_a/f10): suggested bucket: other (minor visual inconsistency with
Settings' per-field edited dot specification).; confidence: confirmed (established via source and contract analysis; not
executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g50_custom_agents_gui_settings_a/f10`.
