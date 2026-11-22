# Add host Tailscale checkbox disables during agent arming pause and while checking, evicting focus

Reviewed commit: f247754a50574cbd4517c9871625dc171e8df000

## TLDR

In Add host, pressing Enter while focused on the Tailscale host checkbox moves keyboard focus away while Farhelm checks
the destination. When reviewing an agent's host-add request, arriving approval cards in the background also temporarily
disable the checkbox, drop keyboard focus from it, and make Tab skip over it while the destination and alias fields stay
editable.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/hosts/add.rs:1593`.

In `crates/gui/farhelm-gui/src/hosts/add.rs:1440-1441,1483,1592-1601`, `AddHostDialog::render` passes
`busy = self.busy(cx)` (defined at `add.rs:915-917` as
`self.probing(cx) || self.pending().is_some_and(|request| request.paused(cx))`) to `ssh_fields`, which binds
`.disabled(busy)` and guards `on_toggle` with `if !dialog.busy(cx)`. Meanwhile, `destination` and `alias` remain
editable during `request.paused(cx)` (only locked via `set_read_only(true)` while an Allow answer, probe, or sign-in is
actually in flight, i.e. `probing(cx)`), and `add-host-submit` uses `.busy(busy)` specifically so it keeps focus
(`add.rs:1519-1521`, `SPEC_impl.md`, "Drafts"). Because `Checkbox::disabled(true)` stops calling `focus::track`
(`crates/gui/farhelm-gui/src/components/checkbox.rs:307-313`): (1) during an agent host-add dialog, any background
approval list change triggers the 700ms arming pause (`approvals/answers.rs:85`), disabling the checkbox, dropping focus
if the user is on it (violating `SPEC.md`, "Keyboard focus", "Things changing underneath"), and making Tab skip it; and
(2) pressing Enter on the focused Tailscale checkbox starts a probe and immediately evicts focus from the checkbox. Fix
direction: gate the checkbox's non-editable state on `self.probing(cx)` (not `request.paused(cx)`), and keep it
focusable/read-only while probing rather than dropping focus tracking via `.disabled(true)`.

Reviewer metadata (g46-tailscale-gui-and-repairs/f1): suggested bucket: other (cannot be highest: no security,
credential, or user-data loss); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g46-tailscale-gui-and-repairs/f1`.
