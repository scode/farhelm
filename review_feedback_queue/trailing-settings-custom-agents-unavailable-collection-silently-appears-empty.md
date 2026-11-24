# Unavailable CustomAgents collection is silently ignored, making a failed or corrupted catalog appear empty

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

If the helm fails to read the custom-agents catalog (for example due to a database error or a corrupted catalog row),
Settings hides the failure and shows an empty Custom agents section with only '+ Add custom agent', leaving the user
with no explanation for why their agents disappeared or why re-adding an existing ID fails as already taken.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/custom_agents.rs:61`.

On the helm side, HelmStore::custom_agents (crates/farhelm-helm/src/custom_agents.rs:27-38) explicitly fails the GET
/api/custom-agents read on a corrupted entry rather than dropping it, causing ui_channel to project
Collection::CustomAgents with Availability::Unavailable { error }. In
crates/gui/farhelm-gui/src/settings/custom_agents.rs, sync_custom_agents (lines 116-121) returns early whenever
availability is not Available without surfacing the error, custom_sections (lines 61-78) ignores
meta(Collection::CustomAgents) entirely and returns only whatever ordered::<CustomAgent>() holds (empty on first load,
or silently stale after a refresh failure), and Editor::render (lines 625-705) renders no catalog-unavailable warning.
Compare settings/recipes.rs:492-495, settings/containers.rs:613-615, and settings/recipes/places.rs:100-102, which all
surface Availability::Unavailable { error } through HostText failure lines instead of pretending the list is empty. This
violates SPEC.md, 'Errors and diagnostics' ('Every failed operation surfaces a concrete, actionable error in the
client'). How to verify: inspect custom_agents.rs:61-78, 114-121, 625-705 alongside
farhelm-helm/src/custom_agents.rs:27-38 and settings/recipes.rs:484-497. Fix: check
collections.meta(Collection::CustomAgents) for Availability::Unavailable { error } and display the helm's error as a
HostText failure line (e.g., via choices::failure_outcome) in the Custom agents pages when the catalog cannot be read.

Reviewer metadata (g50_custom_agents_gui_settings_a/f15): suggested bucket: other (diagnostic/error-visibility gap on a
rare catalog-read failure; does not lose or overwrite stored data because the helm still preserves existing rows and
refuses conflicting creates).; confidence: confirmed (established via source and contract analysis; not executed).;
possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g50_custom_agents_gui_settings_a/f15`.
