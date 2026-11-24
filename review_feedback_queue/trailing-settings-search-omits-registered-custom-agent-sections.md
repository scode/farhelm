# Settings Search omits registered custom agent sections, shows '+ Add custom agent' as a result group heading, and tests the nav button instead of page body

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

Searching in Settings for a registered custom agent's display name or ID always returns no matches and hides every
registered custom agent from the Settings sidebar, even though each registered custom agent is its own Settings section.
In addition, searching "custom" labels the result group "+ Add custom agent" (including the sidebar button's "+" prefix)
instead of "Add custom agent".

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/search.rs:434`.

SPEC.md ("Session list", Settings search) specifies: "The **Search** field above the section list finds settings by
section, label or help text. While a query is active, the list shows the sections with matches under their group
headings and gives each section's count." Each registered custom agent is listed as its own section in
`SettingsDialog::navigation` (crates/gui/farhelm-gui/src/settings/frame.rs:210-221), which checks
`self.search.section_count(&section)` for every registered `Section::CustomAgent(id)`. However,
`crates/gui/farhelm-gui/src/settings/search.rs:173-448` (`catalog(platform)`) builds a static list from `Section::all()`
alone and leaves `Section::CustomAgent(_) => {}` at line 440. Because `Collection::CustomAgents` is already pushed into
`HelmStore`, searching registered custom agent sections would not mount pages or query hosts, yet
`section_count(&Section::CustomAgent(_))` is always `0`: typing a registered agent's name or ID yields "No matches", and
typing "custom" hides all registered custom agent sections while matching only `Section::AddCustomAgent`. Two related
defects in the same search integration: (1) `settings/search.rs:691,695` renders the search result group heading with
`section.label()` (`"+ Add custom agent"`), whereas `settings/frame.rs:385-386` strips `+ ` to `"Add custom agent"` for
the page heading; (2) `settings/search.rs:435` sets the catalog entry's `id` to `section.button_id()`
(`"settings-section-add-custom-agent"`), which is the always-present sidebar nav button rather than a page-body element
(such as `"custom-agent-command"`), so `every_catalog_entry_corresponds_to_something_drawn`
(crates/gui/farhelm-gui/src/settings/search/tests.rs:516-525) passes without checking that the Add custom agent page
drew any control. How to verify: inspect settings/search.rs:173-448,531-558,688-698 and settings/frame.rs:210-221. Fix:
dynamically match pushed `CustomAgent` entries (by display name, ID, and command/help, rendering custom agent names via
`HostText`/`HostSentence`) alongside static catalog entries, use `"Add custom agent"` for the group heading, and target
a real body element ID in the `AddCustomAgent` catalog entry.

Reviewer metadata (g51_custom_agents_gui_settings_b/f4): suggested bucket: other (Settings search discoverability gap
and minor heading/test-coverage defect; registered custom agents remain reachable by scrolling the sidebar when search
is cleared).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g51_custom_agents_gui_settings_b/f4`.
