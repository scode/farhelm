# Templates editor hides custom Effort values as 'Default' and refuses valid agentless custom-effort modifier templates

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

When a launch template has a custom effort value without an agent type (a valid modifier template) or after switching
its agent type from a custom agent to a built-in agent, the Templates editor displays Effort as 'Default' instead of
showing the stored custom value, refuses to save, and (when Effort renders as a dropdown) ignores clicking 'Default' so
the user cannot reset it directly. Unchecking Agent type in Save as template for a custom agent with custom effort is
likewise refused.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/templates/editor/rows.rs:326`.

Three places disagree with `check_template_shape` and hide stored custom effort values:

1. `crates/farhelm-proto/src/launcher.rs:570-581` (and test `free_text_effort_needs_a_compatible_agent` at lines
   649-673) explicitly allows a template with `agent: None` and a valid `LaunchEffortValue::Custom(effort)` so a
   modifier template can carry a custom effort for an earlier stacked custom-agent template.
2. `crates/gui/farhelm-gui/src/templates/fields.rs:280-293` (`Field::refusal`) checks
   `if self == Field::Effort && !custom && let Some(Some(chosen)) = &fields.effort && !chosen.known().is_some_and(...)`,
   where `custom` is `fields.agent.is_some_and(LaunchHarness::uses_custom_catalog)`. When `fields.agent` is `None`,
   `custom` is `false` and `chosen.known()` is `None`, so `Field::refusal` refuses the template with
   `'this agent type or model does not offer this effort; change or remove it'`. This blocks saving both in the
   Templates editor (`editor.rs:735`) and in Save as template (`save_panel.rs:158`) when the user unchecks Agent type to
   save a custom-effort modifier template.
3. In `crates/gui/farhelm-gui/src/templates/editor.rs:674-681` (`sync_inputs`) and
   `crates/gui/farhelm-gui/src/templates/editor/rows.rs:323-339`, whenever `fields.agent` is `None` or a built-in
   harness, `Field::Effort` skips the text input and renders `self.choice(field, options, chosen, ...)` with
   `let chosen = fields.effort.as_ref().and_then(Option::as_ref).and_then(|value| value.known());`. For any
   `LaunchEffortValue::Custom(s)`, `chosen` collapses to `None`, which matches the `(None, "Default")` option! Thus:
   - The stored custom effort `s` is never shown (not even as `"<s> (not offered)"`, breaking `rows.rs:435` /
     `rows.rs:443`).
   - The UI shows `'Default'` as selected right above the refusal alert saying the effort is not offered.
   - Whenever `effort_options` has more than `MAX_SEGMENTS` (5) entries — which is always true when `fields.agent` is
     `None` (9 options: Default + 8 entries in `LaunchEffort::ALL`), and also for built-in agents/models offering 5+
     efforts — `choice` renders a `Select` (`rows.rs:468`), whose `choose` handler (`components/select.rs:685`) ignores
     selecting the already-chosen value (`"0"` = Default), so clicking Default does nothing.

How to verify: (a) On New session with a custom agent and custom effort `deep_work`, open Save as template, uncheck
Agent type, leave Effort checked, and save — `SavePanel::refusal` rejects it via `fields.rs:280` even though
`check_template_shape` accepts it. (b) In the Templates editor, create an Agent template with a custom agent and Effort
`deep_work`, then switch Agent type to Claude (or open a stored template with `agent: None, effort: "deep_work"`):
Effort displays 'Default' while refusing save, and selecting 'Default' in the dropdown does not clear `deep_work`.

Fix: (1) In `fields.rs:280-293`, when `fields.agent.is_none()` and `fields.effort` is
`Some(Some(LaunchEffortValue::Custom(s)))`, validate with `validate_model_or_effort(s)` instead of refusing it as
unknown (matching `check_template_shape`). (2) In `editor.rs:674-681` and `rows.rs:323-339`, pass
`Option<LaunchEffortValue>` (rather than collapsing via `.known()`) to `choice` for built-in agents so an incompatible
`LaunchEffortValue::Custom(s)` appears as `"<s> (not offered)"` and choosing `'Default'` actually resets `fields.effort`
to `Some(None)`; and for `fields.agent.is_none()` with a custom effort, allow viewing/editing it via the text input or
showing the literal value.

Reviewer metadata (g54_custom_agents_gui_templates_goldens/l4): suggested bucket: high — blocks saving valid agentless
custom-effort modifier templates (including via Save as template), hides incompatible custom effort values behind
'Default' while refusing save, and prevents resetting to Default via the dropdown. Not `highest` (no security or
user-data loss).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as
`g54_custom_agents_gui_templates_goldens/l4`.
