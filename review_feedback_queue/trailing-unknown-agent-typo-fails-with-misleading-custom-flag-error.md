# Unknown --agent typo with --prompt, --permissions guarded, or dotted/slashed --model fails with misleading Custom-harness error instead of unknown_agent

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

When an agent mistypes a built-in agent name (such as `--agent cluade` or `--agent opencode`) together with `--prompt`,
`--permissions guarded`, or a built-in model name containing `.` or `/` (such as `gpt-6.2-sol`), Farhelm refuses the
command with a misleading Custom-agent flag error (e.g., "--prompt/--prompt-file is not offered by this agent") instead
of telling the agent that the agent name is unknown and listing the built-in types.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/agent_launch.rs:256`.

In commit cc8b0208acd3853986a951cabf1c8008f722d6e2 (#4733), `crates/farhelm-helm/src/agent_launch.rs:323-330` added the
`crate::custom_agents::unknown_agent(id)` check AFTER `apply_template(&state, &flags, &context)` at line 256 (and after
the `--cwd`/`--gh` check at lines 258-272). Because `crates/farhelm/src/main.rs:902-914` (`parse_agent_type`) turns any
valid custom-agent ID (like `cluade`, `codxe`, or `opencode`) into
`agent: Some(LaunchHarness::Custom), custom_agent: Some(Some(id))`, `apply_template` at line 256 evaluates the remaining
CLI flags against `LaunchHarness::Custom` before `agent_launch.rs:327` checks whether `id` is actually registered in
`placement.custom_agents`. Specifically, `crates/farhelm-proto/src/launcher.rs` immediately refuses: (1)
`--prompt`/`--prompt-file` at lines 907-915 (`--prompt/--prompt-file is not offered by this agent`), (2)
`--permissions guarded` (or `approve`/`smart_approve`/`chat`) at lines 1037-1044
(`--permissions guarded is not offered by Custom`), and (3) built-in models with `.` or `/` (e.g. `gpt-6.2-sol`,
`opencode/gpt-6.2-sol`) at lines 973-975 (`--model use 1 to 256 lowercase letters, digits, hyphens, or underscores`).
This defeats SPEC.md ("Agent-spawned sessions", lines 4680-4681: "`--agent` given something that is not an agent type is
refused with the list of agent types") and commit cc8b0208's fix for the most common agent-launch combinations (such as
`farhelm agent create --agent cluade --prompt "..." --cwd /work`). How to verify: trace `agent_launch::resolve_with` at
`crates/farhelm-helm/src/agent_launch.rs:247-331` with `edits` containing
`agent: Some(LaunchHarness::Custom), custom_agent: Some(Some("cluade")), prompt: Some("hello")`. Fix: before
`apply_template(&state, &flags, &context)` at `agent_launch.rs:256`, check whether `edits.custom_agent` names an ID
absent from `placement.custom_agents` (when `edits.agent.is_none_or(LaunchHarness::uses_custom_catalog)`) and return
`Err(crate::custom_agents::unknown_agent(id))` immediately, keeping the post-fold check at lines 327-330 for missing IDs
inherited from templates.

Reviewer metadata (g45_custom_agents_helm_registry/f5): suggested bucket: other — misleading diagnostic error on a
mistyped `--agent` flag combined with common launch flags; no security, data-loss, or valid-input UX impact.;
confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g45_custom_agents_helm_registry/f5`.
