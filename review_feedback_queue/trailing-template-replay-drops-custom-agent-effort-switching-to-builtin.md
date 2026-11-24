# Template replay drops a custom agent's effort when switching to a built-in agent, diverging from apply_template and CLI

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

Applying a launch template (or quick-launch sequence) that switches from a custom agent with a standard effort word such
as `high` to a built-in agent like Claude drops the effort to Default in the desktop launcher, whereas the CLI keeps
`high`. Switching from a custom agent with a non-standard effort such as `deep_work` to a built-in agent also clears the
effort silently with no 'effort was cleared' notice.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/templates/apply.rs:122`.

Commit `df0dff4c` updated `templates::apply::snapshot` (`crates/gui/farhelm-gui/src/templates/apply.rs:122`) to read
`effort: agent.effort_value()` so the shared `apply_template` fold sees `draft.agent.custom_effort` (parsed via
`From<String> for LaunchEffortValue`, turning `'high'` into `LaunchEffortValue::Known(LaunchEffort::High)`). When a
template (or a second template in a sequence) then switches from `LaunchHarness::Custom` to a built-in harness such as
`LaunchHarness::Claude` without setting `fields.effort`:

1. `farhelm_proto::launcher::apply_template` (`crates/farhelm-proto/src/launcher.rs:838-853`) passes `next.effort`
   (`Some(Known(High))`) into `reconcile_harness_selection` (`launcher.rs:223-229`), which preserves `High` because
   Claude offers it (so the CLI's `apply_templates` and the fold's `applied.effort` both keep `High`).
2. `templates::apply::replay` (`crates/gui/farhelm-gui/src/templates/apply.rs:509`) replays the agent switch through
   `draft.choose_harness(agent, context.catalog, remembered(agent))`, which builds its input selection via
   `LaunchDraft::choices_for` (`crates/gui/farhelm-gui/src/launcher/draft.rs:958`:
   `effort: agent.effort.map(Into::into)`). Because custom-agent efforts live in `draft.agent.custom_effort` (set at
   `apply.rs:569`) rather than `draft.agent.effort`, `choices_for` feeds `effort: None` into
   `reconcile_harness_selection` and `reconciliation_notice`.
3. Consequently, `draft.agent.effort` becomes `None` instead of `Some(LaunchEffort::High)` (diverging from
   `apply_template` and the CLI, breaking SPEC_impl.md's guarantee that 'the CLI and `tl:` cannot disagree about what a
   template means'), and for a non-standard custom effort like `deep_work`, `reconciliation_notice` sees `None -> None`
   and emits no notice that the deliberate effort was cleared.

How to verify: In `crates/gui/farhelm-gui/src/templates/apply_tests.rs`, start from a draft with custom agent
`acme-code` and `custom_effort = "high"` (or apply a two-template sequence `[custom_with_effort_high, claude_only]`),
then apply `TemplateFields { agent: Some(LaunchHarness::Claude), .. }`. `apply_template` returns
`applied.effort == Some(LaunchEffortValue::Known(LaunchEffort::High))`, while `draft.agent.effort` is `None`.

Fix: Update `LaunchDraft::choices_for` (`crates/gui/farhelm-gui/src/launcher/draft.rs:958`) to read
`effort: agent.effort_value()` (matching `apply::snapshot` at `apply.rs:122`) and
`custom_agent: agent.custom_agent.clone()`, or synchronize the reconciled effort in `apply::replay` as well. Verify both
hand switches and template replay preserve compatible known efforts and emit the reconciliation notice when an
incompatible custom effort is cleared.

Reviewer metadata (g54_custom_agents_gui_templates_goldens/f6): suggested bucket: other — GUI/CLI divergence on a
custom-to-built-in agent switch with effort set; no security or user-data loss.; confidence: confirmed; possible cover:
none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as
`g54_custom_agents_gui_templates_goldens/f6`.
