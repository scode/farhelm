# Claude trust hint band shows on Command and legacy launches in ordinary directories

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

When Claude is started in an ordinary folder through a custom Command launch (or a legacy session) and asks whether to
trust the folder, Farhelm still shows the banner saying Farhelm's Settings switch can answer it for you—even though
turning that switch on has no effect on Command launches in ordinary folders.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/settings/agent_trust.rs:296-309`.

In `crates/farhelm-supervisor/src/service/core.rs:14657-14660` (added in commit
b3a14cd25cc48869e18bc6be867d47822251a15c), `LaunchSpec.claude_workspace_trust` is derived solely from
`selection.is_some_and(|selection| selection.workspace_trust == Some(true) && crate::agent_kind::edits_config_for_workspace_trust(selection.harness))`
(`SPEC_impl.md:3510-3511`). For an ordinary directory (`preparation == None`), literal Command launches
(`SessionLaunch::Command`) and legacy launches have `selection == None`, so enabling Claude's Settings trust switch
never triggers the edit. Previously, `trust_band.rs` gated Claude's hint on `working_copy.is_some()`, where fresh
managed-checkout preparation (`preparation.claude_trust`) applied regardless of launch kind. Commit
b3a14cd25cc48869e18bc6be867d47822251a15c deleted `eligible()`
(`crates/gui/farhelm-gui/src/session_pane/trust_band.rs:108-110, 342`) and `TrustAgent::managed_checkouts_only()`
(`agent_trust.rs`), leaving `TrustAgent::asking` (`crates/gui/farhelm-gui/src/settings/agent_trust.rs:296-309`)
returning `Some(TrustAgent::Claude)` whenever `info.claude_trust_prompt` is true—without checking
`info.launch.agent_selection()` (unlike Codex, Muse, and Antigravity at lines 303-308, which require a structured launch
of that harness and are tested in `trust_band_tests.rs:927`). Note that in a fresh managed checkout
(`info.working_copy.is_some()`), `preparation.claude_trust` still applies to non-Claude or Command launches, so the
mismatch is specifically for Command/legacy launches in ordinary directories (`info.working_copy.is_none()`). Fix
direction: in `TrustAgent::asking` (or `trust_band.rs`), gate `info.claude_trust_prompt` on either
`info.working_copy.is_some()` or a structured Claude launch
(`info.launch.agent_selection().is_some_and(|s| s.harness == LaunchHarness::Claude)`), and update
`only_a_sessions_own_launch_names_the_agent_asking` in
`crates/gui/farhelm-gui/src/session_pane/trust_band_tests.rs:766-814`.

Reviewer metadata (g17-trust-gui-and-goldens/f3): suggested bucket: other (cannot be highest: misleads the user into
enabling a switch that does not affect Command or legacy launches in ordinary directories, with no security bypass or
data loss); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g17-trust-gui-and-goldens/f3`.
