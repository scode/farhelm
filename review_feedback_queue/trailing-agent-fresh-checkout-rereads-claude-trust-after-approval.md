# Agent-approved managed checkouts re-read Claude workspace trust after approval

Reviewed commit: b3a14cd25cc48869e18bc6be867d47822251a15c

## TLDR

When an agent asks to create a session in a new managed checkout, Farhelm shows an approval card stating whether the
workspace will be trusted for Claude. If the Claude trust setting is turned on while the approval card is waiting (or
while a new disposable environment is being prepared), Farhelm re-reads that setting after you approve the Not trusted
card and still marks the cloned repository trusted in Claude's settings file, allowing repository hooks and commands to
run. Conversely, if Claude trust was on when the card was shown and turned off before dispatch, Farhelm still writes
Claude trust for the new checkout but skips recording the cleanup entry, so archiving or deleting the checkout never
removes that trust from Claude's settings.

## Details

**highest.** Source inspection. Main reference `crates/farhelm-helm/src/sessions.rs:2623`.

1. Before showing the approval card, both agent-requested managed checkout paths freeze the launch's
   `LaunchSelection.workspace_trust` from current Settings:
   - Installed host managed checkout: `crates/farhelm-helm/src/agent_requests.rs:2235` (`create_checkout_for_agent`
     calls `normalize_launch_workspace_trust`) -> `approve_launch` (`agent_requests.rs:2361-2379`) ->
     `sessions::create_agent_checkout` (`agent_requests.rs:2390`).
   - Disposable environment with repo: `crates/farhelm-helm/src/agent_requests.rs:4473-4476`
     (`launch_environment_for_agent` calls `configured_launch_workspace_trust`) -> `approved`
     (`agent_requests.rs:4502-4524`) -> `CreateReq::for_agent_environment` (`agent_requests.rs:4478`) ->
     `sessions.rs:1968`.
2. After approval, both paths enter `create_fresh_session_resolved` (`crates/farhelm-helm/src/sessions.rs:2548`). Lines
   2623-2631 re-read `checkout_config_for(state, claim.host)` and call
   `github_checkout_resolution(&req, client_identity.clone(), &config)`, which sets
   `ResolvedGithubCheckout.claude_trust = config.claude_trust` (`sessions.rs:1570`). Meanwhile, lines 2626-2631 keep the
   pre-approval `resolved_mode` (leaving `LaunchSelection.workspace_trust` at its pre-approval value). Toggling
   `claude_trust` does not bump `config_revision` (`checkout_config.rs:219`, `SPEC_impl.md:3505`), so `sessions.rs:1548`
   accepts the preview.
3. Two concrete failures occur when `claude_trust` toggles between pre-approval composition and post-approval dispatch:
   - Off -> On (security / approval-card violation): The approval card shows `Not trusted` (`workspace_trust: None`),
     yet dispatch sends `ResolvedGithubCheckout.claude_trust = true` -> `CheckoutPreparation.claude_trust = true`
     (`crates/farhelm-supervisor/src/service/core.rs:11301`). In the supervisor shim
     (`crates/farhelm-supervisor/src/launch.rs:1993-2006`), `run_checkout_preparation` writes
     `hasTrustDialogAccepted: true` into `~/.claude.json` for the cloned repository before execing Claude, violating
     SPEC_impl.md ('Permission prompts for agent actions': 'What a card shows is what an approval does. Launches are
     resolved fully before the card, and the dispatched launch is the one shown') and SPEC.md (Managed checkouts: 'Trust
     allows repository hooks and commands just as answering Yes does').
   - On -> Off (leaked trust / missing archive cleanup): The approval card shows `Trusted workspace (Settings)`
     (`workspace_trust: Some(true)`), so `LaunchSpec.claude_workspace_trust` is `true` (`core.rs:14664-14667`). Dispatch
     sends `ResolvedGithubCheckout.claude_trust = false` (`preparation.claude_trust = false`).
     `run_checkout_preparation` skips the edit and writes no `ClaudeTrustRecord` into `preparation.state_path`. Then
     `exec_launch_spec_with_seam` (`launch.rs:1217-1235`) sees
     `spec.claude_workspace_trust && !spec.preparation.as_ref().is_some_and(|prep| prep.claude_trust)` and calls
     `apply_claude_workspace_trust(Path::new("."), |_| Ok(()))`, marking the fresh checkout trusted in `~/.claude.json`
     with NO removal record so archiving/deleting the checkout never removes it.
4. Fix direction: Capture the checkout's frozen `claude_trust` alongside `TrustResolvedLaunch` before the approval card
   (or derive `ResolvedGithubCheckout.claude_trust` from the pre-approval snapshot/approved state for agent-origin
   creates, and ensure non-Claude agent checkouts also freeze `claude_trust` before approval if `claude_trust` applies
   to them). Verify with a helm unit test that toggles `claude_trust` between `approve_launch` and dispatch in both
   directions and asserts that `ResolvedGithubCheckout.claude_trust` matches the pre-approval choice.

Reviewer metadata (g15-trust-claude-specs-helm-sup/f1): suggested bucket: highest (could be in highest: yes — breaks
approval-card trust binding so an agent-requested managed checkout approved as Not trusted still marks the repository
trusted in Claude's configuration and allows repository hooks/commands to run); confidence: confirmed; possible cover:
none.

Additional source record at `b3a14cd25cc48869e18bc6be867d47822251a15c`, `crates/farhelm-helm/src/sessions.rs:2623-2631`
(g16-trust-sessions-and-claude-guard/l6):

Two agent-approved fresh-checkout paths carry a pre-approval `TrustResolvedLaunch` (`resolved_mode: Some(...)`) into
`create_fresh_session_resolved` (`crates/farhelm-helm/src/sessions.rs:2548-2689`):

- `create_checkout_for_agent` (`crates/farhelm-helm/src/agent_requests.rs:2235-2405`) -> `create_agent_checkout`
  (`sessions.rs:2751-2796`) -> `create_fresh_session_resolved`.
- `launch_environment_for_agent` (`agent_requests.rs:4473-4524`) with `plan.repo.is_some()` ->
  `CreateReq::for_agent_environment` (`sessions.rs:1306-1350`) -> `finish_session_launch`
  (`provisioning/tensorlake_add.rs:1121`) -> `create_in_session_sandbox` (`sessions.rs:1872-1910`) ->
  `create_session_request` (`sessions.rs:1966-1982`) -> `create_fresh_session_resolved` (spanning both the approval wait
  and environment provisioning). Inside `create_fresh_session_resolved` (`sessions.rs:2623-2631`), `resolved_mode`
  preserves the pre-approval `LaunchSelection.workspace_trust`, but line 2623 re-reads
  `checkout_config_for(state, claim.host)` and line 2624 passes that fresh `config` into `github_checkout_resolution`
  (`sessions.rs:1570`), setting `ResolvedGithubCheckout.claude_trust = config.claude_trust`. Because `set_claude_trust`
  / `set_agent_trust` (`crates/farhelm-helm/src/checkout_config.rs:1348-1370`) deliberately does not bump
  `config_revision`, `preview.binding.config_revision != config.config_revision` (`sessions.rs:1548`) still passes. Both
  toggle directions fail:

1. Off at approval -> On before dispatch: `resolved_mode` has `workspace_trust: None` (as shown on the card, or another
   agent like Codex), yet `ResolvedGithubCheckout.claude_trust` becomes `true`, so the supervisor's
   `run_checkout_preparation` (`crates/farhelm-supervisor/src/launch.rs:1993-2006`) writes
   `hasTrustDialogAccepted: true` to `~/.claude.json` despite the user approving a non-trusted launch.
2. On at approval -> Off before dispatch (for a Claude launch): `resolved_mode` retains `workspace_trust: Some(true)`
   (`LaunchSpec.claude_workspace_trust = true` at `crates/farhelm-supervisor/src/service/core.rs:14664-14667`), while
   `ResolvedGithubCheckout.claude_trust` becomes `false` (`CheckoutPreparation.claude_trust = false` at
   `service/core.rs:11301`). In the shim, `run_checkout_preparation` skips writing the trust key and its cleanup record
   (`launch.rs:1993`), and then `exec_launch_spec_with_seam` (`launch.rs:1217-1235`) sees
   `spec.claude_workspace_trust && !spec.preparation.as_ref().is_some_and(|prep| prep.claude_trust)` and runs the
   ordinary-folder `apply_claude_workspace_trust(Path::new("."), |_| Ok(()))` with NO archive removal record — leaving
   permanent Claude trust behind after the managed checkout is archived (violating `SPEC.md:1774-1775` and
   `SPEC_impl.md:3513-3514, 3521-3522`). Unaffected paths verified: user `create_session`, `replace_with_fresh_checkout`
   (`sessions.rs:4934-4981`), and user `create_in_session_sandbox` all pass `resolved_mode: None` and derive both
   `github_checkout.claude_trust` and `mode` from the single `config` read at `sessions.rs:2623`;
   `reconcile_agent_checkout` (`sessions.rs:2695-2719`) replays the supervisor's already-accepted fingerprint directly.
   Fix direction: Freeze `claude_trust` alongside `resolved_mode` before `approve_launch` (e.g. in `FreshCreateContext`
   and `AgentEnvironmentCreate`) and pass that frozen boolean into `github_checkout_resolution` instead of re-reading
   `config.claude_trust` after approval.

Reviewer metadata (g16-trust-sessions-and-claude-guard/l6): suggested bucket: highest (could be in highest: security —
toggling Claude trust while an agent-requested fresh-checkout approval card waits or while an agent disposable
environment provisions can either write Claude workspace trust into ~/.claude.json for a checkout approved with trust
off, or write Claude trust permanently with no archive removal record when approved with trust on and toggled off before
dispatch); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g15-trust-claude-specs-helm-sup/f1`,
`g16-trust-sessions-and-claude-guard/l6`.
