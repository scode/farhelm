# Host settings Tailscale switch omits action prefix on launch refusal, stacks old errors, and leaves launch refusal stuck

Reviewed commit: f247754a50574cbd4517c9871625dc171e8df000

## TLDR

In Host settings, toggling the Tailscale host switch while a session launch is in progress shows a lowercase sentence
fragment ("not started, a session launch is in progress; try again when it finishes") without naming the setting. If a
previous Tailscale toggle failed, the old error stays displayed above the launch error; and if a Tailscale toggle was
already saving when a launch started and the user clicks it again, the launch-refused error remains stuck on screen even
after the in-flight toggle succeeds.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/hosts/settings.rs:957`.

In `crates/gui/farhelm-gui/src/hosts.rs:179-186`, `LAUNCH_IN_FLIGHT` is explicitly a lowercase suffix meant to follow an
action prefix (e.g. `settings.rs:246,1100` formats `"Destination: {}"`, verified by `settings/tests.rs:495`). In
`crates/gui/farhelm-gui/src/hosts/settings.rs:960-966`, `tailscale_row` renders `super::LAUNCH_IN_FLIGHT` directly with
no `"Tailscale host: "` prefix (and `settings/tests.rs:542-543` pins the bare fragment). Second, in
`settings.rs:759-765`, when `super::launch_in_flight(cx)` is true, `toggle_tailscale` sets
`self.tailscale_launch_refused = true` and returns without suppressing or clearing `self.tailscale.draft.refusal()`, so
`tailscale_row` (`settings.rs:957-966`) renders both `host-settings-tailscale-error` and
`host-settings-tailscale-launch` simultaneously. Third, `settings.rs:762` calls
`self.refused.record(TAILSCALE_SWITCH, &Ok(()))`, which clears `self.refused` for any other control in the dialog even
though no save started on the host key (`settings/refused.rs:48-49,65` provides `Refused::clear`). Fourth (added in Turn
2): because `Work::Save` is not in `Work::changes_host_registry()`, a session launch can start while a Tailscale save is
already in flight; clicking the switch then sets `self.tailscale_launch_refused = true`, and unlike `Destination` (which
clears `self.launch_refused` in `leave_editor` on 200 OK at `settings.rs:662`), `tailscale_launch_refused` is never
cleared when the in-flight save settles, leaving the red launch-refused alert visible beneath a switch that just
succeeded. Fix direction: prepend `"Tailscale host: "` before `LAUNCH_IN_FLIGHT`, suppress `tailscale.draft.refusal()`
while `tailscale_launch_refused` is shown, clear `tailscale_launch_refused` when a Tailscale save settles, and use
`self.refused.clear(TAILSCALE_SWITCH)`.

Reviewer metadata (g46-tailscale-gui-and-repairs/f2): suggested bucket: other (cannot be highest: diagnostic/alert
wording and stale error display); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g46-tailscale-gui-and-repairs/f2`.
