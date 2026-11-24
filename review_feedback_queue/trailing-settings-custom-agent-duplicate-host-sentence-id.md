# Section::display_label emits duplicate flat DriverNode element_id 'custom-agent-name-{id}' in Settings nav and page heading in the same frame

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

Whenever a registered custom agent's Settings page is open, both its sidebar navigation row and its page heading emit a
label with the same flat element ID in GPUI's driver node list, violating Farhelm's unique-ID rule for host sentences
and making element-ID lookups in tests and the gpui-agent automation bridge ambiguous.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/frame.rs:290`.

Reconciled against GPUI runtime internals (vendor/gpui-pre) and gpui-agent (vendor/gpui-agent): GPUI's `GlobalElementId`
hashes the full ancestor `element_id_stack` (vendor/gpui-pre/src/window.rs:1035,1068) and AccessKit `NodeId` derives
from `GlobalElementId::accesskit_node_id` (vendor/gpui-pre/src/window/a11y.rs:55-56), so GPUI runtime rendering, element
state, and AccessKit node IDs do NOT collide across the different parent containers (`settings-section-custom-{id}` vs
`settings-heading`). However, `Window::driver_push_node` (vendor/gpui-pre/src/window.rs:4353-4355) sets
`DriverNode.element_id` from `global_id.and_then(|id| id.last())` — the leaf `ElementId` alone, discarding ancestor
scoping — and `gpui-agent` exports that flat leaf ID directly as `RawNode.id`
(vendor/gpui-agent/gpui/gpui-agent/src/lib.rs:393). Because both `SettingsDialog::section_button`
(crates/gui/farhelm-gui/src/settings/frame.rs:290,299) and `SettingsDialog::page_heading`
(crates/gui/farhelm-gui/src/settings/frame.rs:388,445) call `section.display_label(cx)` in the same
`SettingsDialog::render` frame, and `Section::display_label`
(crates/gui/farhelm-gui/src/settings/custom_agents.rs:44-56) hardcodes
`HostSentence::new(SharedString::from(format!("custom-agent-name-{id}")))`, the driver tree contains two `Role::Label`
nodes with identical flat `element_id` `"custom-agent-name-{id}"`. This violates `HostSentence`'s documented rule
(crates/gui/farhelm-gui/src/host_text.rs:328-329: "two clones drawn in one frame are two nodes with one id; a sentence
is cloned to be kept ..., not to be drawn twice") and causes flat `element_id` queries in `test_support.rs:638` and
`gpui-agent` to collide on the first node (the nav button label) instead of addressing nav and heading distinctly. How
to verify: inspect window.rs:4353-4355, gpui-agent/src/lib.rs:393, settings/frame.rs:290,299,388,445, and
settings/custom_agents.rs:44-56. Fix: parameterize `display_label` with a slot/prefix (e.g. `"nav"` vs `"heading"`,
producing `settings-nav-custom-name-{id}` and `settings-heading-custom-name-{id}`).

Reviewer metadata (g51_custom_agents_gui_settings_b/l2): suggested bucket: other (driver/automation element-ID collision
and violation of host_text.rs uniqueness rule; GPUI rendering state and AccessKit NodeIds are unaffected; no security or
data-loss impact).; confidence: confirmed; possible cover: none.

Additional source record at `df0dff4c9b925534b0aae763c325155279a58829`,
`crates/gui/farhelm-gui/src/settings/custom_agents.rs:53` (g50_custom_agents_gui_settings_a/l4):

Revised from Turn 1 candidate after reconciling with g51/l2 and checking
crates/gui/farhelm-gui/src/host_text.rs:318-319,328-329 and crates/gui/farhelm-gui/src/test_support.rs:635-640.
HostSentence explicitly specifies that it is 'exposed to assistive technology (and to the agent bridge's element list)
as one label node under its id' and warns: 'A clone draws the same parts under the same id, so two clones drawn in one
frame are two nodes with one id; a sentence is cloned to be kept (inside a frozen Question, say), not to be drawn
twice.' Flat element lookups such as test_support::label (test_support.rs:635-640) match on flat node.element_id without
ancestor scoping. At crates/gui/farhelm-gui/src/settings/custom_agents.rs:44-56, Section::display_label unconditionally
constructs HostSentence::new(SharedString::from(format!("custom-agent-name-{id}"))). Both SettingsDialog::section_button
(settings/frame.rs:290) and SettingsDialog::page_heading (settings/frame.rs:388) call section.display_label(cx) in the
same render frame when a custom agent is selected, producing two nodes with identical element_id
'custom-agent-name-{id}'. (The other two sub-parts of the original l4 lead — Section::button_id/key collisions and
render-time sync_custom_agents convergence — remain refuted.) How to verify: inspect custom_agents.rs:44-56,
settings/frame.rs:290,388, host_text.rs:318-329, and test_support.rs:635-640. Fix: have display_label take a caller/slot
discriminator (e.g. 'custom-agent-nav-name-{id}' vs 'custom-agent-heading-name-{id}') or pass the element ID from
section_button and page_heading.

Reviewer metadata (g50_custom_agents_gui_settings_a/l4): suggested bucket: other (duplicate element ID in
accessibility/test-bridge element list; no security, data-loss, or direct visual breakage).; confidence: confirmed
(established via source and contract analysis; not executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g50_custom_agents_gui_settings_a/l4`,
`g51_custom_agents_gui_settings_b/l2`.
