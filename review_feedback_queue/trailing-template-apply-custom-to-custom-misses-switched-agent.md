# Applying a template that switches between two custom agents fails to mark switched_agent, leaving an earlier escalation banner open

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

If you switch in the launcher from a guarded agent (like Codex) to a custom agent — raising the red
permission-escalation banner with Undo — and then apply a launch template that selects a different custom agent, the old
escalation banner stays open instead of closing. Clicking Undo on that stale banner then reverts past both the earlier
switch and the template you just applied.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/templates/apply.rs:219`.

At reviewed tip 1b3fe49af86979aebc2719965682a17f8c6366ac (in `crates/gui/farhelm-gui/src/templates/apply.rs:213-239`,
touched by mandatory patch 165_7164b111_s1.patch and df0dff4c9b925534b0aae763c325155279a58829):

- The batch replay loop records `let harness = draft.agent.harness;` before `replay(...)` (line 219) and checks only
  `if draft.agent.harness != harness` (line 228) to set `last_switch` and thus `Applied::switched_agent` (line 239).
- Because all registered custom agents share `LaunchHarness::Custom`, applying a template that changes
  `draft.agent.custom_agent` from `Some("agent-a")` to `Some("agent-b")` leaves `draft.agent.harness` unchanged
  (`Some(LaunchHarness::Custom)`), so `switched_agent` is `false`.
- By contrast, switching custom agents by hand in `LaunchDraft::choose_custom_agent`
  (`crates/gui/farhelm-gui/src/launcher/draft.rs:760-766`) explicitly sets `edit.switched_agent = true` when
  `custom_agent` changes, which dismisses any prior escalation banner.
- In `Launcher::apply_now` (`crates/gui/farhelm-gui/src/launcher/view.rs:1092`),
  `self.follow_escalation(switched_agent, escalation, notice_before, cx)` keeps an already-showing escalation banner
  open when `switched_agent` is `false`. That directly violates `templates/apply.rs:181-183` ("Some step of the batch
  changed the agent type, even if a later step changed it back. A banner an earlier action raised goes away then, since
  its Undo would also take back this batch.").
- Note: Other parts of initial lead `l2` were checked and found handled: `Launcher::refresh_custom_capabilities` runs on
  `render`, `press_at`, and `submit` (`launcher/view.rs:1336, 1425, 2279`), and `remove_held`
  (`templates/apply.rs:385-398`) matches `Option<Option<String>>` properly because generic Custom templates store
  `Some(None)` (`save.rs:95`, `rows.rs:422`, `main.rs:1107-1111`).
- How to verify: Unit-test `templates::apply::apply` starting from a draft on custom agent `"acme-code"` and applying a
  template setting `agent: Some(LaunchHarness::Custom), custom_agent: Some(Some("other-code".into()))`, and check
  `applied.switched_agent` (currently `false`, should be `true`).
- Fix: Capture both `let harness = draft.agent.harness;` and `let custom = draft.agent.custom_agent.clone();` before
  `replay(...)` in `templates/apply.rs:219`, and treat
  `(draft.agent.harness != harness || draft.agent.custom_agent != custom)` as an agent switch (setting
  `switched_agent = true` and updating `last_switch` when `harness` changed).

Reviewer metadata (g49_custom_agents_gui_plumbing/l2): suggested bucket: other (narrow UX/state-consistency issue:
leaves a stale escalation Undo banner open after a custom-to-custom template switch; no security or data-loss impact).;
confidence: confirmed; possible cover: none.

Additional source record at `df0dff4c9b925534b0aae763c325155279a58829`,
`crates/gui/farhelm-gui/src/templates/apply.rs:219` (g54_custom_agents_gui_templates_goldens/l1):

In `crates/gui/farhelm-gui/src/templates/apply.rs:219-231`, `apply` tracks whether a template batch switched agents by
comparing only `draft.agent.harness` before and after `replay`
(`let harness = draft.agent.harness; ... if draft.agent.harness != harness`). All registered custom agents share
`LaunchHarness::Custom`, so switching from custom agent A to custom agent B via a template leaves `draft.agent.harness`
unchanged and returns `Applied { switched_agent: false, .. }`. By contrast, picking a different custom agent by hand in
`LaunchDraft::choose_custom_agent` (`crates/gui/farhelm-gui/src/launcher/draft.rs:760-767`) compares
`self.agent.custom_agent` and sets `edit.switched_agent = true`. `Launcher::follow_escalation`
(`crates/gui/farhelm-gui/src/launcher/view/escalation.rs:81`) relies on `switched_agent` to dismiss any prior escalation
banner because its Undo snapshot predates the new agent switch (matching `apply.rs:181-183`: 'Some step of the batch
changed the agent type, even if a later step changed it back. A banner an earlier action raised goes away then, since
its Undo would also take back this batch.').

How to verify: In `crates/gui/farhelm-gui/src/templates/apply_tests.rs:157-172`
(`custom_agent_template_replays_the_saved_registration_and_effort`), capture the `Applied` returned by the second
`fixture.apply` switching from `acme-code` to `other-code` and assert `applied.switched_agent` — it currently returns
`false`.

Fix: Snapshot both `let harness = draft.agent.harness;` and `let custom_agent = draft.agent.custom_agent.clone();`
before `replay` in `apply.rs:219`, and treat
`draft.agent.harness != harness || draft.agent.custom_agent != custom_agent` as an agent switch (setting `last_switch` /
`switched_agent`).

Reviewer metadata (g54_custom_agents_gui_templates_goldens/l1): suggested bucket: other — minor UX inconsistency; an
open 8-second escalation banner is not dismissed across a custom-to-custom template switch, so pressing Undo on it also
undoes the template batch's draft edits (unsaved form input only, no security or user-data loss).; confidence:
confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g49_custom_agents_gui_plumbing/l2`,
`g54_custom_agents_gui_templates_goldens/l1`.
