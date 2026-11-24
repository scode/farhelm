# Templates editor lets user save generic 'Custom (not offered)' on an Agent launch template

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

Switching a Command template with generic 'Custom' to an Agent template shows 'Custom (not offered)' in the Agent type
dropdown, yet the editor still allows saving it without an error. The saved template cannot launch on its own and clears
any previously chosen custom agent when applied.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/templates/fields.rs:238`.

Generic `Custom` (`agent: Some(LaunchHarness::Custom), custom_agent: Some(None)` or `None`) is valid only as a Command
launch's declared agent (SPEC.md, 'Topology': 'The Command tab can declare a registered agent or generic Custom';
`launcher/request.rs:337-338` refuses any Agent launch with `harness == Custom && custom_agent.is_none()` with
`'Choose a registered custom agent before launching'`). In the Templates editor:

1. `crates/gui/farhelm-gui/src/templates/editor/rows.rs:302-319` correctly omits `(LaunchHarness::Custom, None)` from
   `options` when `fields.kind != Some(LauncherKind::Command)`, so when a Command template with generic Custom is
   switched to Agent (`editor.rs:491`), `rows.rs:316` displays `"Custom (not offered)"`.
2. However, `Field::refusal` (`crates/gui/farhelm-gui/src/templates/fields.rs:229-295`) only checks
   `self == Field::Agent && shell && fields.kind == Some(LauncherKind::Command)` (line 238); it never checks
   `self == Field::Agent && custom && fields.kind == Some(LauncherKind::Agent) && fields.custom_agent.as_ref().and_then(Option::as_ref).is_none()`.
3. Neither `Draft::refusal` nor `check_template_shape` refuses this combination, so Save succeeds in violation of
   SPEC.md ('Incompatible fields stay visible and prevent saving until corrected or removed'). Applying the resulting
   template sets `harness = Some(Custom)` and explicitly clears `custom_agent = None`
   (`farhelm-proto/src/launcher.rs:894`), and `quick_launch_eligible` (`farhelm-proto/src/launcher.rs:418-426`) still
   considers it quick-launch eligible because `fields.agent.is_some()`.

How to verify: In the Templates editor, create a Command template, add Agent type, select generic 'Custom', switch
'Switches launcher to' to 'Agent' (observing 'Custom (not offered)'), and click Save. Save succeeds with
`kind: agent, agent: custom, custom_agent: null`.

Fix: In `Field::refusal` (`crates/gui/farhelm-gui/src/templates/fields.rs:238-243`), return a refusal when
`self == Field::Agent && custom && fields.kind == Some(LauncherKind::Agent) && fields.custom_agent.as_ref().and_then(Option::as_ref).is_none()`
(e.g., explaining that generic Custom is only for Command launches and asking the user to choose a registered custom
agent or another agent type).

Reviewer metadata (g54_custom_agents_gui_templates_goldens/f5): suggested bucket: other — allows saving an incompatible
Agent-tab generic Custom template after switching launch kind even though the control labels it '(not offered)'. No
security or user-data loss.; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as
`g54_custom_agents_gui_templates_goldens/f5`.
