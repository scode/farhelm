# Launcher::choose_effort omits sync_fields, leaving the custom effort input out of sync with the draft

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

When resetting a custom agent's effort to default via launcher search (`effort:default`), the launcher's custom effort
text field is never resynchronized with the draft, so the visible text input can retain stale custom effort text.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view/choices.rs:112-116`.

`LaunchHarness::Custom::offers_effort()` (`crates/farhelm-proto/src/launch.rs:344-353`) returns `true`, so launcher
search (`crates/gui/farhelm-gui/src/launcher/search.rs:640-650`) offers `Row::Effort(None)` ("Effort: Default") when a
custom agent is selected and the user types `effort:default`. Accepting that row in
`crates/gui/farhelm-gui/src/launcher/view/search.rs:555` calls `self.choose_effort(effort, cx)`.

In `crates/gui/farhelm-gui/src/launcher/view/choices.rs:112-116`:

```rust
pub(super) fn choose_effort(&mut self, effort: Option<LaunchEffort>, cx: &mut Context<Self>) {
    self.draft.choose_effort(effort);
    self.notice = None;
    self.changed(cx);
}
```

Unlike `chose`, `choose_permissions`, `take_recent`, `reset_choices`, `apply_now`, and `undo_escalation`,
`Launcher::choose_effort` does not take `window: &mut Window` and never calls `self.sync_fields(window, cx)`. As a
companion to `draft.rs:894-896` (`LaunchDraft::choose_effort` not clearing `self.agent.custom_effort`, owned by `g52`),
even when `draft.choose_effort(None)` clears `self.agent.custom_effort`, `self.custom_effort` (`TextInputState`) is
never synced by `Launcher::choose_effort` and continues displaying the stale custom effort text until some later
unrelated action triggers `sync_fields`.

How to verify: Inspect `crates/farhelm-proto/src/launch.rs:344-353`,
`crates/gui/farhelm-gui/src/launcher/search.rs:640-650`, `crates/gui/farhelm-gui/src/launcher/view/search.rs:555`, and
`crates/gui/farhelm-gui/src/launcher/view/choices.rs:112-116`.

Fix: Pass `window: &mut Window` to `Launcher::choose_effort` (updating its callers in `choices.rs` and `view/search.rs`)
and call `self.sync_fields(window, cx)` alongside clearing `self.agent.custom_effort` in `LaunchDraft::choose_effort`.

Reviewer metadata (g53_custom_agents_gui_launcher_b/f7): suggested bucket: other — Correctness/UI sync bug companion to
`g52/f1` when clearing custom effort via `effort:default` search; no security or data-loss impact.; confidence:
confirmed (source analysis of `choices.rs:112-116`, `view/search.rs:555`, `draft.rs:894-896`,
`launcher/search.rs:640-650`, and `farhelm-proto/src/launch.rs:344-353`; not executed).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g53_custom_agents_gui_launcher_b/f7`.
