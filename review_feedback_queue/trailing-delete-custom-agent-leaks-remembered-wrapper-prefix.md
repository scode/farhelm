# Deleting a custom agent leaves its remembered wrapper prefix in preferences, leaking it onto other or newly registered custom agents

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

When a user launches a custom agent with a wrapper prefix and later deletes that custom agent, Farhelm keeps the wrapper
prefix remembered and silently pre-fills it when selecting another or newly registered custom agent.

## Details

**high.** Source inspection. Main reference `crates/farhelm-helm/src/custom_agents.rs:94`.

In `crates/farhelm-helm/src/store.rs:263-282` (`record_launch_history`), `preferences.remembered_wrapper_prefixes` is
stored as `BTreeMap<LaunchHarness, Vec<String>>`, collapsing all registered custom agents into one
`LaunchHarness::Custom` key (also reported by g44). Alongside that, `HelmStore::delete_custom_agent`
(`crates/farhelm-helm/src/custom_agents.rs:94-106`, added in commit 0ae1f6de79be7b712ec33dfd71b3a38edf3b1399) only runs
`DELETE FROM custom_agents WHERE id = ?1` and performs no cleanup of remembered wrapper prefixes in `preferences`.
Consequently, even if a user deletes the custom agent that used a wrapper prefix and later creates a new custom agent,
`crates/gui/farhelm-gui/src/launcher/view/choices.rs:136` reads `self.remembered_wrapper(LaunchHarness::Custom, cx)` and
pre-fills the deleted agent's wrapper prefix (and if `remembered_wrapper_prefixes` is fixed to key custom agents by
registration ID, `delete_custom_agent` still needs to prune the deleted ID's entry in the same transaction so
re-registering the same ID does not inherit a deleted registration's wrapper prefix or accumulate stale entries in
`preferences`). How to verify: inspect `crates/farhelm-helm/src/custom_agents.rs:94-106`,
`crates/farhelm-helm/src/store.rs:255-287`, and `crates/gui/farhelm-gui/src/launcher/view/choices.rs:81-141`. Fix: key
remembered custom-agent wrapper prefixes by custom agent ID (rather than shared `LaunchHarness::Custom`) and remove the
deleted ID's entry inside `HelmStore::delete_custom_agent` in the same SQLite transaction.

Reviewer metadata (g45_custom_agents_helm_registry/f4): suggested bucket: high — silently pre-filling a deleted or
sibling custom agent's wrapper prefix onto a different custom agent can break launches or run an unintended wrapper
command (note: this shares root cause with g44's `f1` on `store.rs:263`; the user-visible wrapper command is still shown
in the launcher before Launch and any custom agent is already YOLO, so it is `high` rather than `highest`).; confidence:
confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g45_custom_agents_helm_registry/f4`.
