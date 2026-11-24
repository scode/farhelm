# Launcher::target names a declared custom agent on the Command tab outside Restart with, unlike built-in agents

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

When creating, cloning, or replacing a session on the Command tab with a registered custom agent declared, the main
action's accessible name includes the custom agent's name ("Launch Command · <name> · <host> · <folder>"), whereas
declaring any built-in agent or generic Custom omits the agent name ("Launch Command · <host> · <folder>").

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view.rs:1792-1804`.

In `crates/gui/farhelm-gui/src/launcher/view.rs:1783-1804`, `Launcher::target` is documented as naming "the agent type
(on the Agent tab, or a Restart with command's declared one)" and computes `harness` accordingly:

```rust
let harness = match self.draft.tab {
    Tab::Agent => self.draft.agent.harness,
    Tab::Command if matches!(self.draft.mode, Mode::RestartWith(_)) => {
        self.draft.command.agent
    }
    Tab::Command => None,
};
```

Immediately below (`view.rs:1800-1803`), `custom_agent` omits the `matches!(self.draft.mode, Mode::RestartWith(_))`
guard on `Tab::Command`:

```rust
let custom_agent = match self.draft.tab {
    Tab::Agent => self.draft.agent.custom_agent.as_deref(),
    Tab::Command => self.draft.command.custom_agent.as_deref(),
};
```

Because `TargetWords::sentence` (`view.rs:1745-1749`) renders `self.custom_agent` whenever it is `Some`, a registered
custom agent declared on the Command tab appears in the Launch/Replace button's accessible name outside `RestartWith`,
while built-in agents and generic `Custom` do not.

How to verify: Inspect `crates/gui/farhelm-gui/src/launcher/view.rs:1783-1834` and
`crates/gui/farhelm-gui/src/launcher/view.rs:1745-1749`.

Fix: Gate `Tab::Command` in the `custom_agent` match on `if matches!(self.draft.mode, Mode::RestartWith(_))` to match
`harness` (or update both branches and the doc comment together if Command-tab declarations are meant to be named in all
modes).

Reviewer metadata (g53_custom_agents_gui_launcher_b/f6): suggested bucket: other — Minor accessible-name inconsistency
on the Command tab between registered custom agents and built-in/generic declared agents; no functional, data-loss, or
security impact.; confidence: confirmed (source analysis of `crates/gui/farhelm-gui/src/launcher/view.rs:1745-1834`; not
executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g53_custom_agents_gui_launcher_b/f6`.
