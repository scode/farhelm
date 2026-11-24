# Accepting effort:default in launcher search does not clear a custom agent's typed effort

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

When a registered custom agent is selected in the session launcher and has a typed effort, choosing "Effort: Default"
via composer search (e.g. `effort:default`) appears to accept the choice and clears the search box, but leaves the
custom agent's effort unchanged and still sends it at launch.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/draft.rs:894`.

- `LaunchHarness::Custom.offers_effort()` returns `true` (`crates/farhelm-proto/src/launch.rs:344-354`).
- In `crates/gui/farhelm-gui/src/launcher/search.rs:665-671`, when `inputs.harness == Some(LaunchHarness::Custom)` on
  the Agent tab and the user queries `effort:default` (or `effort:`), search emits `Row::Effort(None)` ("Effort:
  Default").
- Accepting `Row::Effort(None)` calls `Launcher::choose_effort(None, cx)`
  (`crates/gui/farhelm-gui/src/launcher/view/search.rs:562`, `view/choices.rs:112-116`), which calls
  `LaunchDraft::choose_effort(None)` (`crates/gui/farhelm-gui/src/launcher/draft.rs:894-896`).
- `LaunchDraft::choose_effort` only writes `self.agent.effort = effort;`, leaving `self.agent.custom_effort` untouched.
  Because `AgentChoices::effort_value()` (`draft.rs:273-279`) reads `self.custom_effort` whenever
  `harness.uses_custom_catalog()` is true, the custom effort remains set in both the UI field and the launched
  `LaunchSelection`.
- Note also the view-layer companion (interface group `g53_custom_agents_gui_launcher_b`): `Launcher::choose_effort`
  (`view/choices.rs:112-116`) does not call `self.sync_fields(window, cx)`, so even after `LaunchDraft::choose_effort`
  clears `custom_effort`, the `custom_effort` `TextInputState` must be synced in `Launcher::choose_effort` so the
  visible Effort input clears as well.
- How to verify: In `draft_tests.rs`, select a custom agent via `choose_custom_agent`, set
  `draft.agent.custom_effort = "deep".into()`, call `draft.choose_effort(None)`, and observe
  `draft.agent.effort_value()` is still `Some(Custom("deep"))`. In a view test, type `effort:default` with a custom
  agent selected and accept it.
- Fix: In `LaunchDraft::choose_effort`, when `self.agent.harness.is_some_and(LaunchHarness::uses_custom_catalog)` and
  `effort.is_none()` (or whenever `effort` is updated), clear `self.agent.custom_effort` (or update it from
  `effort.map(LaunchEffort::as_cli_arg).unwrap_or_default()`), and have `Launcher::choose_effort` call `sync_fields`.

Reviewer metadata (g52_custom_agents_gui_launcher_a/f1): suggested bucket: other (minor correctness/convenience bug;
user can still clear the Effort text input manually).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g52_custom_agents_gui_launcher_a/f1`.
