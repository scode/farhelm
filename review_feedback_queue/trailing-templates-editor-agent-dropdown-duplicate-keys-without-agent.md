# Templates editor Agent type dropdown emits duplicate option keys and displays wrong agent when custom_agent is set without agent

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

Opening a stored template that sets a custom agent ID without an agent type (valid under the helm's save rules) breaks
the Agent type dropdown in the Templates editor: it adds a duplicate 'unavailable' entry with the same internal key as
the real registered agent, prevents clicking the real registered agent to fix the template, and can panic in debug
builds with assistive technology active. A template that only clears custom_agent (`custom_agent: null`, no `agent`) is
falsely shown as Cursor.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/templates/editor/rows.rs:312`.

`check_template_shape` (`crates/farhelm-proto/src/launcher.rs:560-569`) permits templates where `fields.agent` is `None`
and `fields.custom_agent` is `Some(Some(id))` or `Some(None)` (it only rejects `Some(Some(id))` when
`fields.agent.is_some_and(|a| !a.uses_custom_catalog())`). In the Templates editor:

1. `Field::Agent.is_set` (`crates/gui/farhelm-gui/src/templates/fields.rs:120`) returns `true` whenever
   `fields.agent.is_some() || fields.custom_agent.is_some()`.
2. `crates/gui/farhelm-gui/src/templates/editor/rows.rs:312` computes
   `let chosen = (fields.agent.unwrap_or(LaunchHarness::BUILT_INS[0]), fields.custom_agent.clone().flatten());`. Because
   `LaunchHarness::BUILT_INS[0]` is `LaunchHarness::Cursor`, `fields.agent == None` is silently turned into
   `LaunchHarness::Cursor`.
3. When `fields.custom_agent == Some(Some(id))` and `id` is registered:
   - `options` holds `((LaunchHarness::Custom, Some(id)), "<Display> · <id>")`, while `chosen` is
     `(LaunchHarness::Cursor, Some(id))`.
   - `rows.rs:313` sees `!options.iter().any(|(key, _)| key == &chosen)` and appends a second entry
     `((LaunchHarness::Cursor, Some(id)), "{id} (registration unavailable)")` even though `id` is registered.
   - `agent_choice` (`rows.rs:386-391`) builds each `SelectOption`'s key solely from `id` when `id.is_some()`
     (`format!("custom:{id}")`), ignoring the `LaunchHarness` element of the tuple. Both options therefore receive the
     identical key `"custom:{id}"`.
   - In `SelectList::render` (`crates/gui/farhelm-gui/src/components/select.rs:634-654`), both entries get the same GPUI
     element ID (`"templates-agent-option-custom:{id}"` — a debug panic under assistive technology per `SPEC_impl.md`,
     'GPUI cookbook: traps'), share a single `FocusHandle` in `SelectList::handles`, and both render as checked
     (`is_chosen == true`). The closed select displays the first match (`"<Display> · <id>"`), masking that
     `fields.agent` is `None`, and clicking `"<Display> · <id>"` in the dropdown is ignored by `SelectList`
     (`select.rs:685`: `if choices.chosen.as_ref() != Some(value)`) because both entries share `"custom:{id}"`, so the
     user cannot pick the registered agent without first switching to a different agent.
4. When `fields.custom_agent == Some(None)` and `fields.agent == None`, `chosen` becomes
   `(LaunchHarness::Cursor, None)`, falsely displaying 'Cursor' as chosen.

How to verify: Load a template with `kind: Some(Agent), agent: None, custom_agent: Some(Some("acme-code"))` while
`acme-code` is registered in `Collection::CustomAgents`, open it in the Templates editor, and inspect the
`templates-agent` Select options and keys (or open the dropdown and click `Acme Code · acme-code`).

Fix: Represent `chosen` in `rows.rs:312` with `Option<LaunchHarness>` (instead of `.unwrap_or(BUILT_INS[0])`), include
the harness (or `none`) in the option key in `agent_choice` (`rows.rs:386-391`) so keys are guaranteed unique across
`(Option<LaunchHarness>, Option<String>)`, and show a distinct label (e.g. `"{id} (agent type not set)"` /
`"Custom agent reset (agent type not set)"`) when `fields.agent` is `None`.

Reviewer metadata (g54_custom_agents_gui_templates_goldens/l3): suggested bucket: high — breaks the Agent type selector
with duplicate element IDs (debug panic under assistive tech), false UI state, and a no-op click when trying to select
the registered custom agent. Not `highest` (no security or user-data loss).; confidence: confirmed; possible cover:
none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as
`g54_custom_agents_gui_templates_goldens/l3`.
