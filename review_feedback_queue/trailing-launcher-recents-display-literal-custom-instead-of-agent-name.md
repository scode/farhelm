# Recent-setup rows display literal 'Custom' instead of the custom agent's name or ID

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

In both the launcher's Recent list and typed search results (wide pane and narrow popover alike), every recent setup for
a registered custom agent shows the generic label "Custom" instead of that agent's display name or ID, so users cannot
visually tell different custom agents apart in recent setups without hovering.

## Details

**high.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view/search/start.rs:487-492`.

Both visible renderers for recent-setup rows hardcode `harness_label(selection.harness)` and ignore
`selection.custom_agent`:

1. Wide command pane & wide typed search (`crates/gui/farhelm-gui/src/launcher/view/search/start.rs:487-492`,
   `recent_choice`):

```rust
HostSentence::new("launcher-recent-agent")
    .text(harness_label(selection.harness))
    .underline_matches(query)
```

2. Narrow search popover (`crates/gui/farhelm-gui/src/launcher/view/search.rs:1087-1092`, `recent_lines`):

```rust
let first = first
    .text(" · ")
    .host(placement)
    .text(" · ")
    .text(super::harness_label(selection.harness))
    .underline_matches(query);
```

For any registered custom agent (`LaunchHarness::Custom`), `harness_label` returns `"Custom"`. By contrast,
`launcher/recents.rs:344-348` (`summary_without_wrapper`, used for hover help and accessible names) uses
`selection.custom_agent.as_deref().unwrap_or_else(|| harness_label(selection.harness))`, and `view.rs:1828`,
`view.rs:1859`, and `view.rs:2015` resolve the registration's current display name (falling back to the saved ID when
deleted) via `crate::harness::custom_name(id, collections)`.

Because `recent_choice` is used for both the empty command pane's Recent section (`start.rs:308`) and wide typed
search's `Row::Recent` (`start.rs:566`), and `recent_lines` is used for narrow typed search's `Row::Recent`
(`view/search.rs:1010`), every custom-agent recent setup visually renders as "Custom" and also fails to underline query
matches against the custom agent's name or ID.

How to verify: Inspect `crates/gui/farhelm-gui/src/launcher/view/search/start.rs:487-492`,
`crates/gui/farhelm-gui/src/launcher/view/search.rs:1087-1092`, and
`crates/gui/farhelm-gui/src/launcher/recents.rs:344-348`. Launch two distinct registered custom agents, reopen the
launcher, and inspect the Recent rows in the command pane and search results (both wide and narrow).

Fix: Pass `collections` (or the resolved `HostText` agent name) into both `recent_choice` (`search/start.rs`) and
`recent_lines` (`view/search.rs`) so that when `selection.custom_agent` is `Some(id)`, both rows display
`crate::harness::custom_name(id, collections)` as host text (with `.underline_matches(query)`) instead of literal
`"Custom"`.

Reviewer metadata (g53_custom_agents_gui_launcher_b/f3): suggested bucket: high — Materially degrades usability whenever
a user registers more than one custom agent, since recent setups in the launcher all appear visually identical
("Custom"), making it easy to pick and launch the wrong custom agent from recents (pressing Enter on a highlighted
recent setup in the empty pane launches immediately). Does not bypass security checks or directly lose user data.;
confidence: confirmed (source analysis of `search/start.rs:308`, `search/start.rs:487-492`, `search/start.rs:566`,
`view/search.rs:1010`, `view/search.rs:1087-1092`, and `launcher/recents.rs:344-348`; not executed).; possible cover:
none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g53_custom_agents_gui_launcher_b/f3`.
