# stacked_quick_launch_eligible marks an Agent-tab Custom template without a catalog ID as quick-launch eligible

Reviewed commit: 7164b111339f33f9ec0b296488c38102191f6d9b

## TLDR

A launch template on the Agent tab that selects the generic Custom agent type without naming a registered custom agent
ID can be marked as the quick-launch template (⚡) or treated as ready to quick-launch, even though Farhelm cannot
launch an Agent-tab custom session without a registered agent ID.

## Details

**other.** Source inspection. Main reference `crates/farhelm-proto/src/launcher.rs:408-429`.

In crates/farhelm-proto/src/launcher.rs:408-429, stacked_quick_launch_eligible (and quick_launch_eligible at lines
396-398) tracks only kind, agent, and command, returning agent.is_some() for LauncherKind::Agent. Unlike built-in
harnesses, LaunchHarness::Custom on the Agent tab requires a custom_agent catalog ID (enforced by
SessionLaunch::validate_new at crates/farhelm-proto/src/session_launch.rs:514-517 and sessions::resolve_launch_request
at crates/farhelm-helm/src/sessions.rs:2876-2879). A template with agent: Some(LaunchHarness::Custom) and custom_agent:
None or Some(None) passes check_template_shape (crates/farhelm-proto/src/launcher.rs:560-569) and is directly reachable
via CLI `farhelm agent template create <name> --agent custom --quick-launch`
(crates/farhelm/src/main.rs:902-907,1107-1116 sets agent=Custom, custom_agent=Some(None);
crates/farhelm-helm/src/agent_requests.rs:3315 skips the catalog lookup because custom_agent is Some(None);
agent_requests.rs:3190 checks quick_launch_eligible and accepts it) as well as PUT /api/templates/{name}. Similarly,
stacking a second template with custom_agent: Some(None) clears the catalog ID while stacked_quick_launch_eligible still
returns true, causing quick-launch dispatch (crates/gui/farhelm-gui/src/templates/dispatch.rs:332,421) to attempt an
unseen launch that fails immediately instead of opening New with the needs-more-choices banner.

How to verify: Call quick_launch_eligible(&TemplateFields { agent: Some(LaunchHarness::Custom), ..Default::default() })
and stacked_quick_launch_eligible over [agent=Custom+custom_agent=Some(Some("acme")), custom_agent=Some(None)]; both
return true.

Fix: Track custom_agent alongside agent in stacked_quick_launch_eligible (clearing it whenever agent switches to a
non-custom harness, and updating/clearing it when kind == LauncherKind::Agent and fields.custom_agent is Some(...)). For
LauncherKind::Agent, return agent.is_some_and(|harness| !harness.uses_custom_catalog() || custom_agent.is_some()).

Reviewer metadata (g41b_custom_agents_proto_launch/l2): suggested bucket: other (UX/correctness bug: allows an
unlaunchable partial template to take the quick-launch mark or fail on unseen quick launch instead of showing the
needs-more-choices banner; no security or data-loss impact).; confidence: confirmed; possible cover: none.

Additional source record at `1b3fe49af86979aebc2719965682a17f8c6366ac`,
`crates/farhelm-helm/src/agent_requests.rs:3190,3315-3360` (g46_custom_agents_helm_requests/c4):

- Revised from Turn 1 candidate `c4` after cross-group reconciliation with `g41b/l2` and inspecting
  `crates/gui/farhelm-gui/src/templates/dispatch.rs:332,421` and `crates/farhelm-proto/src/launcher.rs:408-429`.
- When an agent runs `farhelm agent template create foo --agent custom --quick-launch` (with no `--command`), the CLI
  (`crates/farhelm/src/main.rs:1107-1116`) sends `kind: None`, `agent: Some(LaunchHarness::Custom)`,
  `custom_agent: Some(None)`. In `crates/farhelm-helm/src/agent_requests.rs:3315-3360` (`write_template_for_agent`),
  `with_launch_kind` infers `kind: Some(LauncherKind::Agent)`, the `Some(Some(id))` lookup is skipped,
  `check_template_shape` succeeds, and `apply_template_shortcuts` (`agent_requests.rs:3190`) checks
  `farhelm_proto::launcher::quick_launch_eligible(&template.fields)`.
- `stacked_quick_launch_eligible` (`crates/farhelm-proto/src/launcher.rs:408-429`) checks only
  `LauncherKind::Agent => agent.is_some()` without checking that a `Custom` harness has a `custom_agent` ID
  (`Some(Some(_))`). Unlike catalog-dependent validation, an Agent-tab `Custom` selection with no `custom_agent` ID is
  statically incomplete and can never launch (`crates/farhelm-helm/src/sessions.rs:2876-2879` unconditionally rejects it
  with `"select a registered custom agent"`; `SPEC.md`, line 4709, allows `--agent custom` without a registration only
  for command launches).
- Consequences:
  1. `farhelm agent template create foo --agent custom --quick-launch` is permitted to claim (or move) the ⚡
     quick-launch mark (`SPEC.md`, line 4767: "⚡ needs a template that chooses an agent or a nonblank command after the
     write").
  2. In the GUI, `crates/gui/farhelm-gui/src/templates/dispatch.rs:421` (`on_the_page`) sees `launches == true` and
     returns `Prepared::Launch`, attempting an unseen direct quick launch that fails immediately with a refusal alert
     (`Not launched. select a registered custom agent`) instead of opening New with the incomplete-template banner
     (`SPEC.md`, lines 1374-1375). `dispatch.rs:332` (`pick_from_switcher`) likewise omits the incomplete banner.
- How to verify: inspect `crates/farhelm-proto/src/launcher.rs:408-429`,
  `crates/farhelm-helm/src/agent_requests.rs:3190,3315-3360`, and
  `crates/gui/farhelm-gui/src/templates/dispatch.rs:332,421`.
- Fix: track `custom_agent` in `stacked_quick_launch_eligible` (`crates/farhelm-proto/src/launcher.rs:408-429`) so
  `LauncherKind::Agent` requires `agent.is_some_and(|a| !a.uses_custom_catalog() || custom_agent.is_some())`, and in
  `write_template_for_agent` (`agent_requests.rs:3315-3360`) consider refusing an Agent-kind template that explicitly
  clears `custom_agent` to `None` while setting `agent: Custom`.

Reviewer metadata (g46_custom_agents_helm_requests/c4): suggested bucket: other (quick-launch eligibility and
template-write correctness/UX bug; no security or data-loss impact).; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g41b_custom_agents_proto_launch/l2`,
`g46_custom_agents_helm_requests/c4`.
