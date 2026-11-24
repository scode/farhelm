# Agent tab omits a deleted custom agent from harness chips instead of keeping its ID visible

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

When the Agent tab holds a deleted custom agent (for example from Clone, Replace with, a recent setup, a template, or a
deletion while the launcher is open), no harness chip appears selected and the deleted agent's ID is not shown anywhere
in the form, even though the form still shows custom-agent fields and refuses launch with "The selected custom agent is
no longer registered".

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view/choices.rs:349-391`.

`AgentChoices::custom_registration_missing` (`crates/gui/farhelm-gui/src/launcher/draft.rs:240-241`) states: "The
composer keeps a deleted ID visible so the user knows which choice to repair." On the Command tab
(`crates/gui/farhelm-gui/src/launcher/view.rs:2139-2148`), `launcher-command-agent` explicitly calls
`agent_select.retained(...)` to keep a deleted `command.custom_agent` ID visible.

On the Agent tab, however, `Launcher::harness_choices` (`crates/gui/farhelm-gui/src/launcher/view/choices.rs:349-391`)
builds the Custom chip group solely from `self.store.read(cx).collections().ordered::<CustomAgent>()`. When
`self.draft.agent.custom_agent` is `Some(deleted_id)` and `deleted_id` is absent from `Collection::CustomAgents`:

1. No chip is rendered for `deleted_id` (and if no other custom agents are registered, the entire Custom section is
   omitted).
2. `.chosen(Some(HarnessChoice::Custom(deleted_id)))` (`choices.rs:388-391`) matches no chip in the `RadioGroup`, so the
   harness row shows no selection at all.
3. Meanwhile `Launcher::agent_fields` (`view.rs:1871-1877`) still renders `custom_agent_fields(palette)` (Model, Effort,
   YOLO), and `agent_refusal` (`request.rs:341-342`) only reports "The selected custom agent is no longer registered"
   without naming the missing ID.

How to verify: Inspect `choices.rs:349-391`, `view.rs:1871-1877`, `view.rs:2139-2148`, and `draft.rs:240-241`. Clone a
session that ran a now-deleted custom agent (or delete the custom agent while it is selected in New) and inspect the
Agent tab's harness chips.

Fix: When `self.draft.agent.custom_agent` is `Some(id)` and `collections.get::<CustomAgent>(id)` is `None`, render a
retained/unavailable chip (or visible status line naming `id` as host text) in `harness_choices` so the selected deleted
ID stays visible until the user chooses another agent.

Reviewer metadata (g53_custom_agents_gui_launcher_b/f4): suggested bucket: other — UX/correctness gap on the Agent tab
when a selected custom agent is deleted (no security or data-loss impact, and launch is still refused).; confidence:
confirmed (source analysis of `choices.rs:349-391`, `view.rs:1851-1895`, `view.rs:2139-2148`, and `draft.rs:240-241`;
not executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g53_custom_agents_gui_launcher_b/f4`.
