# Custom agent drafts retain a stale built-in effort field that leaks onto built-in agents and causes false effort-cleared notices

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

When cloning, replacing, or loading a recent setup for a custom agent whose effort matches a built-in effort word (such
as "high" or "low"), or switching from a built-in agent to a custom agent before the model catalog loads, editing or
clearing Effort in the launcher leaves a hidden stale effort behind. Switching to a built-in agent like Codex or Claude
then unexpectedly picks up that old effort (even if you cleared it), and applying a model-setting template while still
on the custom agent displays a false warning saying your effort was cleared even though it was not.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/draft.rs:646`.

At reviewed tip 1b3fe49af86979aebc2719965682a17f8c6366ac (from mandatory patch 165_7164b111_s1.patch / commit
7164b111339f33f9ec0b296488c38102191f6d9b, lines 55 and 79):

- `crates/gui/farhelm-gui/src/launcher/draft.rs:629-646` (`LaunchDraft::restore_selection`) populates both
  `agent.custom_effort` and `agent.effort = selection.effort.as_ref().and_then(|value| value.known())` even when
  `selection.harness.uses_custom_catalog()` is true. For any custom agent launch using a word in `LaunchEffort` (`off`,
  `minimal`, `low`, `medium`, `high`, `xhigh`, `max`, `ultra` — e.g. `high` from the custom-agent contract docs),
  `LaunchEffortValue` decodes as `Known(...)`, so `agent.effort` is set alongside `agent.custom_effort`.
- A second entry path exists in `crates/gui/farhelm-gui/src/launcher/draft.rs:983-996` (`LaunchDraft::reconcile`): when
  `catalog.is_none()` and `harness.offers_effort()` (true for `LaunchHarness::Custom` at
  `crates/farhelm-proto/src/launch.rs:353`), line 985 sets `after.effort = effort` and line 995 stores
  `self.agent.effort = after.effort.as_ref().and_then(|value| value.known())`, while `choose_custom_agent`
  (`draft.rs:764`) only clears `self.agent.custom_effort`.
- Editing or clearing the custom agent's Effort text input (`crates/gui/farhelm-gui/src/launcher/view.rs:887`) only
  writes `launcher.draft.agent.custom_effort`, leaving `launcher.draft.agent.effort` untouched.
- Consequence 1: Switching from the custom agent to a built-in harness calls `choices_for` (`draft.rs:952-965`), which
  reads `agent.effort` (line 958) instead of `effort_value()` and passes the stale `LaunchEffort` into
  `reconcile_harness_selection`, carrying an old/cleared effort onto the built-in harness (or triggering a spurious
  effort-cleared notice on harnesses without effort).
- Consequence 2: Applying a template that sets a model on that custom agent calls `draft.choose_custom_model`
  (`draft.rs:848-860`) -> `clear_incompatible_effort(LaunchHarness::Custom, catalog)` (`draft.rs:1002-1011`). Because
  `compatible_efforts(LaunchHarness::Custom, ...)` is empty, it clears the hidden `self.agent.effort` and returns a
  misleading notice `"The selected effort is not in Farhelm's offering for that model or harness, so it was cleared"`
  even though `agent.custom_effort` is unchanged and still sent by `effort_value()` (`draft.rs:273-279`).
- How to verify: Unit-test `LaunchDraft::clone_of` (or `restore_recent`) with `LaunchHarness::Custom` and
  `effort: Some(LaunchEffort::High.into())`, clear `draft.agent.custom_effort`, then call
  `draft.choose_harness(LaunchHarness::Codex, Some(&catalog), None)` and observe
  `draft.agent.effort == Some(LaunchEffort::High)`; or call `draft.choose_custom_model("fast-v2", Some(&catalog))` and
  observe a non-None effort-cleared notice while `draft.agent.custom_effort` remains `"high"`.
- Fix: Keep `agent.effort = None` whenever `harness.uses_custom_catalog()` (in `restore_selection`, `reconcile` — don't
  preserve built-in `effort` across `catalog.is_none()` into `uses_custom_catalog()` — and `choose_custom_agent`), and
  skip `clear_incompatible_effort` when `harness.uses_custom_catalog()`.

Reviewer metadata (g49_custom_agents_gui_plumbing/l6): suggested bucket: other (minor correctness/UX bug: stale effort
leaks across harness switches and produces a misleading cleared-effort notice; no security or data-loss impact).;
confidence: confirmed; possible cover: none.

Additional source record at `df0dff4c9b925534b0aae763c325155279a58829`,
`crates/gui/farhelm-gui/src/launcher/draft.rs:958` (g52_custom_agents_gui_launcher_a/f2):

- `LaunchDraft::choices_for` (`crates/gui/farhelm-gui/src/launcher/draft.rs:952-965`) constructs the pre-switch
  `LaunchSelection` for `reconcile_harness_selection` using `effort: agent.effort.map(Into::into)` (line 958) rather
  than `effort: agent.effort_value()`.
- Meanwhile, for custom agents, `AgentChoices::effort_value()` (`draft.rs:273-279`) and `templates::apply::snapshot`
  (`templates/apply.rs:122`) treat `agent.custom_effort` as the source of truth, and typing in the custom Effort field
  updates only `draft.agent.custom_effort` (`view.rs:887`), never `draft.agent.effort`.
- Two bugs result:
  1. Typed custom effort ignored on switch: Selecting a custom agent, setting `custom_effort = "high"` (or
     `"deep-work"`), and switching to Claude, Grok, or Bash passes `effort: None` into `reconcile_harness_selection`.
     For Claude, `"high"` is silently lost instead of preserved (compare `apply_template` which snapshots
     `effort_value()`). For Grok/Cursor/OpenCode/Bash, `reconciliation_notice` (`draft.rs:1567-1611`) sees
     `before.effort == None` and emits no notice that the user's effort was cleared.
  2. Stale restored effort resurrected on switch: `LaunchDraft::restore_selection` (`draft.rs:632-646`) sets BOTH
     `custom_effort: "high"` AND `effort: Some(LaunchEffort::High)` when restoring a custom agent launch with a known
     effort word (via Clone, Replace with, or Recent setup). If the user then clears `custom_effort` (to `""`) or edits
     it to `"low"` and switches to Claude, `choices_for` still passes stale `agent.effort = Some(High)`, so Claude
     unexpectedly receives `effort: Some(High)` (and switching to Bash falsely reports clearing an effort that the user
     had already cleared).
- How to verify: In `draft_tests.rs`: (a) select custom agent, set `custom_effort = "high"`, call
  `choose_harness(LaunchHarness::Claude, Some(&catalog), None)`, observe `draft.agent.effort` is `None`; (b) restore a
  custom agent recent setup with `effort: Some(LaunchEffort::High.into())`, clear `draft.agent.custom_effort.clear()`,
  call `choose_harness(LaunchHarness::Claude, Some(&catalog), None)`, observe `draft.agent.effort` resurrects
  `Some(LaunchEffort::High)`.
- Fix: Use `effort: agent.effort_value()` in `LaunchDraft::choices_for` (`draft.rs:958`), and in `restore_selection`
  (`draft.rs:646`) leave `effort: None` when `selection.harness.uses_custom_catalog()` so `agent.effort` and
  `agent.custom_effort` do not hold conflicting values (and clear `self.agent.effort = None` in `choose_custom_agent` if
  `reconcile` retained one on an unread catalog — see `draft.rs:983`).

Reviewer metadata (g52_custom_agents_gui_launcher_a/f2): suggested bucket: other (surprising effort drop/resurrection
and missing/false cleared-effort notice when switching away from a custom agent; visible in the Effort control before
launch).; confidence: confirmed; possible cover: none.

Additional source record at `df0dff4c9b925534b0aae763c325155279a58829`,
`crates/gui/farhelm-gui/src/launcher/draft.rs:983` (g52_custom_agents_gui_launcher_a/f7):

- In `crates/gui/farhelm-gui/src/launcher/draft.rs:969-998` (`LaunchDraft::reconcile`), `reconcile_harness_selection`
  (`crates/farhelm-proto/src/launcher.rs:161-169`) deliberately clears `selection.effort = None` whenever switching from
  a built-in harness to `LaunchHarness::Custom` (independent of the model catalog, because custom agents have no catalog
  entries).
- Immediately after calling `reconcile_harness_selection`, `draft.rs:983-986` executes:
  ```rust
  if catalog.is_none() && harness.offers_effort() {
      // An unread catalog cannot make an effort incompatible.
      after.effort = effort;
  }
  ```
- Because `LaunchHarness::Custom.offers_effort()` returns `true` (`crates/farhelm-proto/src/launch.rs:353`),
  `after.effort` is overwritten back to the old built-in effort whenever `catalog.is_none()`.
- As a result: (1) `reconciliation_notice` (`draft.rs:991`) sees `before.effort == after.effort` and omits the
  cleared-effort notice even though `custom_effort` is cleared to `""` (`draft.rs:835, 764`) and the custom agent
  launches with no effort; (2) `self.agent.effort` retains the old `Some(LaunchEffort)` value hidden behind
  `custom_effort == ""`, so switching to another built-in agent later resurrects that effort via `choices_for`
  (`draft.rs:958`, see also `f2`).
- How to verify: In `draft_tests.rs`, select Claude with `effort = Some(LaunchEffort::High)`, call
  `draft.choose_custom_agent(&agent, None, None)`. Observe `edit.notice` does not mention effort being cleared,
  `draft.agent.custom_effort` is `""`, and `draft.agent.effort` is still `Some(LaunchEffort::High)`.
- Fix: Guard `draft.rs:983` with `!harness.uses_custom_catalog()` (i.e.
  `if catalog.is_none() && harness.offers_effort() && !harness.uses_custom_catalog()`).

Reviewer metadata (g52_custom_agents_gui_launcher_a/f7): suggested bucket: other (missing cleared-effort notice and
stale hidden effort state when switching to a custom agent with an unread catalog).; confidence: confirmed; possible
cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g49_custom_agents_gui_plumbing/l6`,
`g52_custom_agents_gui_launcher_a/f2`, `g52_custom_agents_gui_launcher_a/f7`.
