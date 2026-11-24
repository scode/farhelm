# Session actions menu header and quick switcher show generic 'Custom agent' instead of registered custom agent display name or fallback ID

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

Sessions launched with a registered custom agent show the generic label "Custom agent" in both the session row's actions
menu header and the quick switcher's agent column, instead of the custom agent's current display name (or its saved ID
after the registration is deleted). Meanwhile the session row's hover summary card and agent-mark tooltip show the
specific agent name.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/session_list/menu.rs:1439`.

SPEC.md ("Topology", custom agents) requires: "The session list uses the current display name and falls back to the
saved ID when the registration is deleted." Commit df0dff4c9b925534b0aae763c325155279a58829 added `harness::custom_name`
(crates/gui/farhelm-gui/src/harness.rs:23-31) and updated `crates/gui/farhelm-gui/src/session_list/row_marks.rs:360-362`
and `crates/gui/farhelm-gui/src/session_list/summary.rs:270-286`, but did not update
`crates/gui/farhelm-gui/src/session_list/menu.rs:1439-1444` (`agent_name`). `menu::agent_name` still inspects only
`row.info.agent_kind.display_name()`, which returns `Some("Custom agent")` for `AgentKind::Custom`
(crates/farhelm-proto/src/lib.rs:1713). That helper feeds both the session row's actions menu header
(`crates/gui/farhelm-gui/src/session_list/menu.rs:1113,1404`) and the quick switcher's agent column
(`crates/gui/farhelm-gui/src/session_list/quick_switcher.rs:1478-1481`). For both structured custom agent launches and
command launches declaring a registered custom agent, users cannot tell which custom agent a session runs from the menu
header or quick switcher, and deleted registrations do not fall back to their saved ID there. How to verify: inspect
session_list/menu.rs:1402-1444 and session_list/quick_switcher.rs:1478-1481 against harness.rs:23-31 and SPEC.md. Fix:
pass `&Collections` to `menu::agent_name(row, collections)`, add a `Custom(HostText)` variant rendered with `.host(...)`
/ `host_text::element(...)`, and check
`row.info.launch.custom_agent().map(|id| crate::harness::custom_name(id, collections))` before falling back to
`row.info.agent_kind.display_name()`.

Reviewer metadata (g51_custom_agents_gui_settings_b/l3): suggested bucket: other (correctness and UX inconsistency
across session-list surfaces for custom-agent users; no security or data-loss impact).; confidence: confirmed; possible
cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g51_custom_agents_gui_settings_b/l3`.
