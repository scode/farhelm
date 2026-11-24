# Settings nav button tooltip concatenates custom agent display name without HostSentence directional isolation

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

Hovering a custom agent's row in the Settings sidebar shows a tooltip that embeds the agent's display name directly in a
plain string instead of isolating its text direction. A custom agent whose display name contains right-to-left
characters (such as Hebrew or Arabic letters) can reorder surrounding words in the tooltip.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/frame.rs:312`.

In crates/gui/farhelm-gui/src/settings/frame.rs:290-314, SettingsDialog::section_button builds
`let display = section.display_label(cx); let label = display.accessible_text();` and passes
`HoverHelp::tooltip(format!("Show {label} settings")).beside_list(false)` to `Button::new`. For
`Section::CustomAgent(id)`, `Section::display_label` (crates/gui/farhelm-gui/src/settings/custom_agents.rs:44-56) wraps
`agent.display_name` (or fallback `id`) in `HostText::from_host` inside a `HostSentence`. Calling
`display.accessible_text()` returns the escaped presentation WITHOUT left-to-right directional isolates (`LRI`/`PDI`,
host_text.rs:26), and `HoverHelp::tooltip` constructs `Line::App(...)` (components/mod.rs:168-170). This violates
SPEC_impl.md, "Host-supplied text" ("Host text is never concatenated with the app's words in any other way. Hover help
may name a host value ... only as a host sentence") and components/mod.rs:145-147. Note that presentation-unsafe
characters (controls and BiDi override/isolate controls) are already escaped by `accessible_text()`, so only `LRI`/`PDI`
directional isolation for ordinary RTL scripts (which `CustomAgent::validate` permits) is missing. How to verify:
inspect settings/frame.rs:290-314 and register a custom agent with RTL display text (e.g. "סוכן 1") to observe the
unisolated `Line::App` tooltip in the driver record. Fix: build the tooltip for `Section::CustomAgent(id)` with
`HoverHelp::sentence(HostSentence::new(...).text("Show ").host(&HostText::from_host(name)).text(" settings")).beside_list(false)`.

Reviewer metadata (g51_custom_agents_gui_settings_b/l1): suggested bucket: other (contract violation of SPEC_impl.md
Host-supplied text causing missing bidi isolation in a non-destructive Settings sidebar tooltip;
presentation-unsafe/spoofing controls are already escaped by accessible_text(), so there is no security or data-loss
route).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g51_custom_agents_gui_settings_b/l1`.
