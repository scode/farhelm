# Switching between two custom agents retains the previous custom agent's wrapper prefix and explicit flag in hand switches and template replay

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

When you switch in the launcher from one registered custom agent to another — either by clicking its chip, picking it in
search, or applying a launch template — Farhelm keeps the previous custom agent's wrapper prefix (and marks it as an
explicit choice) instead of resetting it to the remembered wrapper. Because More options is collapsed by default, a
wrapper prefix cloned or typed for one custom agent can silently run before a different custom agent.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/draft.rs:759`.

At reviewed tip 1b3fe49af86979aebc2719965682a17f8c6366ac (confirming g44's f1 and auditing all `switch_agent` state
across `draft.rs` and `templates/apply.rs`):

- `LaunchDraft::switch_agent` (`crates/gui/farhelm-gui/src/launcher/draft.rs:825-836`) computes
  `let switched_agent = before.agent.harness != Some(harness);` and only runs the agent-switch cleanup block
  (`conversation.clear()`, `agent.wrapper = joined(remembered_wrapper)`, `agent.wrapper_explicit = false`,
  `agent.permission_flags = String::new()`, clearing custom fields) when `switched_agent` is `true`.
- Because all registered custom agents share `LaunchHarness::Custom`, switching from custom agent A to custom agent B
  via `LaunchDraft::choose_custom_agent` (`draft.rs:747-772`, calling `self.choose_harness(LaunchHarness::Custom, ...)`
  at line 759) evaluates `before.agent.harness != Some(harness)` as `false`.
- `choose_custom_agent` lines 760-767 manually clear `model`, `model_owner`, `custom_effort`, and `conversation`, and
  set `edit.switched_agent = true`, but omit resetting `self.agent.wrapper = joined(remembered_wrapper)`,
  `self.agent.wrapper_explicit = false`, `self.agent.permission_flags.clear()`, and `self.agent.effort = None` (built-in
  ghost field, see `l6`).
- The exact same omission occurs in template replay (`crates/gui/farhelm-gui/src/templates/apply.rs:510-530`): applying
  a template that selects a different custom agent
  (`agent: Some(LaunchHarness::Custom), custom_agent: Some(Some("agent-b"))`, with `wrapper_prefix: None`) calls
  `draft.choose_harness(LaunchHarness::Custom, ...)` where `switched_agent` is `false`, and lines 523-529 clear only
  `conversation`, `model`, `model_owner`, and `custom_effort`, leaving agent A's `wrapper` and `wrapper_explicit`
  attached to agent B.
- Additionally, because `reconcile_harness_selection` (`crates/farhelm-proto/src/launcher.rs:161-169`) checks
  `source != harness`, switching between two custom agents produces `edit.notice = None` even when deliberate `model` or
  `custom_effort` choices were cleared (unlike built-in harness switches).
- How to verify: Unit-test `LaunchDraft::choose_custom_agent` (and `templates::apply::apply`) starting from custom agent
  A with `draft.edit_wrapper("env FOR_A=1".into())`, then switching to custom agent B with `remembered_wrapper = None`,
  and observe `draft.agent.wrapper == "env FOR_A=1"` and `draft.agent.wrapper_explicit == true`.
- Fix: When `self.agent.custom_agent.as_deref() != Some(agent.id.as_str())` in `choose_custom_agent` (and when
  `draft.agent.custom_agent != applied.custom_agent` in `templates/apply.rs:523`), also reset
  `self.agent.wrapper = joined(remembered_wrapper)`, `self.agent.wrapper_explicit = false`,
  `self.agent.permission_flags.clear()`, and `self.agent.effort = None` (or share a single switch-cleanup helper
  parameterized by `(harness, custom_agent)`).

Reviewer metadata (g49_custom_agents_gui_plumbing/f9): suggested bucket: high (material UX/correctness failure where a
collapsed More options wrapper prefix from one custom agent silently carries into a different custom agent in both hand
switches and template replay; surfacing `highest` boundary: because wrapper prefixes run through `bash -c` and may
inject credentials or execution-context wrappers intended only for agent A into agent B while More options is collapsed,
triage should judge whether that credential/context crossover crosses into `highest`, though the wrapper is
user-supplied and shown as a pill on the folded More options row).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g49_custom_agents_gui_plumbing/f9`.
