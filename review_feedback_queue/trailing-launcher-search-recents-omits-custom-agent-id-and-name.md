# Launcher search does not match custom-agent recent setups by registration ID or display name

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

Typing a registered custom agent's ID or display name in launcher search (including `recent:<name>`) does not find
recent setups for that custom agent; recent setups only match the literal word `custom`, their folder/repository, or
their model.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/search.rs:721`.

- In `crates/gui/farhelm-gui/src/launcher/search.rs:717-736`, the `Scope::Recent` / `Scope::All` recent-setup matcher
  builds its searchable text as:
  `format!("{} {} {}", destination_label(entry), harness_label(entry.selection.harness), entry.selection.model.as_deref().unwrap_or_default())`.
- For any registered custom agent launch, `entry.selection.harness` is `LaunchHarness::Custom`, so `harness_label(...)`
  is always `"Custom"`, while `entry.selection.custom_agent` (the registration ID) and its display name from
  `inputs.custom_agents` are completely omitted.
- By contrast, `recents::summary_without_wrapper` (`crates/gui/farhelm-gui/src/launcher/recents.rs:360-364`) displays
  `selection.custom_agent` in the recent row's summary, and SPEC.md (Topology -> Custom agents) states registered agents
  are searchable by display name and ID.
- As a result, `recent:acme-code` or `recent:acme` fails to match a recent launch of custom agent `acme-code` ("Acme
  Code"), returning "No recent setup matches" unless the folder or model happens to contain that substring.
- How to verify: Add a `LaunchHistoryEntry` with `LaunchHarness::Custom` and `custom_agent: Some("acme-code")` to
  `search_tests.rs` and run `draft.search("recent:acme-code")` and `draft.search("recent:acme code")`; both return empty
  rows.
- Fix: Include `entry.selection.custom_agent` and, when present in `inputs.custom_agents`, that registration's
  `display_name` (matched with Unicode lowercasing as on line 591/597) in the recent-setup search filter at
  `search.rs:721-729`.

Reviewer metadata (g52_custom_agents_gui_launcher_a/f3): suggested bucket: other (search discoverability gap for
custom-agent recent setups).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g52_custom_agents_gui_launcher_a/f3`.
