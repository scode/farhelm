# refresh_custom_capabilities treats an unavailable or unloaded CustomAgents collection as a deleted registration

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

If the helm's custom-agents catalog is unavailable or has not loaded yet, the launcher treats any selected custom agent
(on the Agent or Command tab) as deleted—blocking launch with "no longer registered" / "was deleted" messages, hiding or
erroring its conversation-ID field, and prompting the user to replace a registration that was not actually deleted.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view.rs:2253-2274`.

In `crates/gui/farhelm-gui/src/launcher/view.rs:2253-2274`, `Launcher::refresh_custom_capabilities` looks up
`collections.get::<CustomAgent>(id)` without checking `collections.meta(Collection::CustomAgents)` for
`Availability::Available`:

```rust
let selected_custom = self
    .draft
    .agent
    .custom_agent
    .as_deref()
    .and_then(|id| collections.get::<CustomAgent>(id));
self.draft.agent.custom_resumption =
    selected_custom.is_some_and(|agent| agent.supports_resumption);
self.draft.agent.custom_reporting =
    selected_custom.is_some_and(|agent| agent.supports_reporting);
self.draft.agent.custom_registration_missing =
    self.draft.agent.custom_agent.is_some() && selected_custom.is_none();
self.draft.command.custom_registration_missing = self
    .draft
    .command
    .custom_agent
    .as_deref()
    .is_some_and(|id| collections.get::<CustomAgent>(id).is_none());
```

The same unchecked lookup appears in `view.rs:2139-2148` (showing the Command tab's `retained` item with tooltip "Choose
Custom to run this command without the deleted registration") and `choices.rs:125-137` (`choose_custom_agent` reporting
"That custom agent is no longer registered").

In `crates/farhelm-helm/src/ui_channel.rs:477-490`, when reading `/api/custom-agents` fails (e.g., on the initial
channel snapshot), the helm pushes `Collection::CustomAgents` with `Availability::Unavailable { error }` and empty
entries (and before any snapshot, `meta(Collection::CustomAgents)` is `None`). Other GUI surfaces explicitly guard
against treating an unavailable collection as deletion: `settings/custom_agents.rs:108-121` checks
`collections.meta(Collection::CustomAgents).is_some_and(|meta| matches!(meta.availability, Availability::Available))`
with the comment "An unavailable collection must not look like a deletion", and `launcher/request.rs:129-145`
(`Recipes::of`) distinguishes `None` (loading), `Availability::Unavailable` (unreadable), and `Availability::Available`.

Because `refresh_custom_capabilities` skips that check, an unavailable or missing `Collection::CustomAgents` snapshot
sets `custom_registration_missing = true`, `custom_resumption = false`, and `custom_reporting = false`, causing:

- Agent tab launches (Clone, Replace with, Restart with, templates, recents) to be refused as "The selected custom agent
  is no longer registered" (`request.rs:341`), and any entered conversation ID to report "This custom agent cannot
  resume now" / "no longer supports resumption" (`request.rs:565-577`).
- Command tab launches to be refused as "The custom agent registration was deleted. Register it again to use Restart
  with." / "Choose Custom to keep using this command." (`request.rs:283-288`) and the dropdown to label it as a deleted
  retained entry (`view.rs:2139-2148`).

How to verify: Inspect `crates/gui/farhelm-gui/src/launcher/view.rs:2139-2148`, `view.rs:2253-2274`,
`crates/farhelm-helm/src/ui_channel.rs:477-490`, `crates/gui/farhelm-gui/src/settings/custom_agents.rs:108-121`, and
`crates/gui/farhelm-gui/src/launcher/request.rs:129-145`. In a test, open Clone or Restart with on a custom-agent
session while `Collection::CustomAgents` has `Availability::Unavailable` (or no meta) and observe the false "deleted" /
"no longer registered" refusals.

Fix: Check `collections.meta(Collection::CustomAgents)` in `refresh_custom_capabilities` (and `view.rs:2139-2148` /
`choices.rs:125-137`) to distinguish loading / `Availability::Unavailable` from a genuinely absent entry in an
`Availability::Available` catalog (paralleling `Recipes::of` and `settings/custom_agents.rs:116-121`), surfacing a
catalog-unavailable/loading state instead of claiming the registration was deleted.

Reviewer metadata (g53_custom_agents_gui_launcher_b/f8): suggested bucket: other — Misleading refusal wording and
false-deleted UI state when `Collection::CustomAgents` is unavailable or unloaded (fails closed, no security or
data-loss impact).; confidence: confirmed (source analysis of `view.rs:2139-2148`, `view.rs:2253-2274`,
`choices.rs:125-137`, `settings/custom_agents.rs:108-121`, `request.rs:129-145`, and
`farhelm-helm/src/ui_channel.rs:477-490`; not executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g53_custom_agents_gui_launcher_b/f8`.
