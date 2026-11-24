# Unseen quick launch with a deleted custom agent wakes a suspended host before refresh_custom_capabilities runs

Reviewed commit: df0dff4c9b925534b0aae763c325155279a58829

## TLDR

Triggering a quick launch (⚡ shortcut, number shortcut, sequence, or Cmd+J) whose template names a deleted custom agent
on a suspended host (such as a suspended Tensorlake sandbox or Podman container) wakes the suspended host anyway, only
failing with "no longer registered" / "was deleted" after the host finishes waking.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/view.rs:394-400`.

`Launcher::refresh_custom_capabilities` (`crates/gui/farhelm-gui/src/launcher/view.rs:2253-2274`) is documented as
"Refresh after catalog pushes and before dispatch, including template launches prepared out of sight", but it is only
called in `Launcher::render` (`view.rs:2280`), `Launcher::press_at` (`view.rs:1336`), and `Launcher::submit`
(`view.rs:1425`).

For a quick launch prepared out of sight (`prepare_unseen` -> `prepared` at `view.rs:451-485` ->
`start_or_wait_for_folder` at `view.rs:394-428`), none of those three methods has run yet when
`start_or_wait_for_folder` invokes `self.begin_host_wake(SourceConsent::default(), mark, window, cx)` (`view.rs:396`).
Inside `Launcher::begin_host_wake` (`view.rs:2962-2990`), line 2969 checks
`if launch::waking(cx) || self.refusal(cx).is_some() { return false; }` specifically to prevent waking a suspended host
for a non-launchable draft. Because `draft.agent.custom_registration_missing` and
`draft.command.custom_registration_missing` remain `false` (and `can_wake(host)` bypasses both `FOLDER_DEFAULT_WAITING`
at `request.rs:272` and `WAIT_FOR_PREVIEW` at `request.rs:425`), `self.refusal(cx)` returns `None` and `begin_host_wake`
calls `launch::wake::start`, waking the suspended sandbox/container host.

Only after the host wakes and `continue_after_wake` eventually reaches `Launcher::submit` (`view.rs:1425`) does
`refresh_custom_capabilities` run, set `custom_registration_missing = true`, and refuse the launch.

Note on cross-group `g54/c3` (`submit_or_wait_for_preview` at `view/checkout.rs:510`): on a connected host with a
managed checkout, `checkout_refusal` (`WAIT_FOR_PREVIEW`) precedes `agent_refusal` / `Tab::Command` refusal in
`LaunchDraft::draft_refusal` (`request.rs:260-290`), so `submit_or_wait_for_preview` would still see `WAIT_FOR_PREVIEW`
even after refreshing capabilities; `submit()` then refreshes capabilities before sending the create. The actual
premature side effect from missing `refresh_custom_capabilities` before `start_or_wait_for_folder` is `begin_host_wake`
waking a suspended host (and, on a non-checkout quick launch with a pending folder default, performing an unnecessary
history read before `submit()` refuses).

How to verify: Inspect `view.rs:394-400`, `view.rs:451-485`, `view.rs:2253-2274`, `view.rs:2962-2990`, and
`request.rs:260-290`. In a GPUI test, create a suspended resumable host (sandbox/container) and a quick-launch template
naming a deleted custom agent on that host; trigger quick launch and observe `launch::wake::start` sending a host wake
instead of immediately revealing the launcher with the missing-registration refusal.

Fix: Call `launcher.refresh_custom_capabilities(cx)` inside `prepared` (`view.rs:460`, right after `finish(...)` applies
templates and before `start_or_wait_for_folder`) and/or at the start of `start_or_wait_for_folder` and
`begin_host_wake`.

Reviewer metadata (g53_custom_agents_gui_launcher_b/f9): suggested bucket: other — Unnecessary side effect (waking a
suspended sandbox/container host before discovering the custom agent is missing); does not launch an unintended agent or
lose user data.; confidence: confirmed (source analysis of `view.rs:394-428`, `view.rs:451-485`, `view.rs:1425`,
`view.rs:2253-2274`, `view.rs:2962-2990`, and `request.rs:260-290`; not executed).; possible cover: none.

Superseded earlier candidate record: `g53_custom_agents_gui_launcher_b/c3`.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g53_custom_agents_gui_launcher_b/f9`.
