# Unverified resumable hosts display the wake checkout allocation notice while Launch refuses

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

When a suspended or stopped sandbox/container host has no verified installation identity yet, the managed-checkout
Folder line promises that the folder will be picked automatically when the host is ready, while Launch is
disabled/refused saying the host installation is not verified.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/request.rs:388-397`.

In `crates/gui/farhelm-gui/src/launcher/request.rs:388-397`, `LaunchDraft::checkout_refusal` checks that the host has a
non-empty `host.identity` BEFORE checking `if self.can_wake(host) { return None; }`, returning `Some(UNVERIFIED_HOST)`
when `host.identity` is absent or empty. However, `Launcher::checkout_allocation_notice`
(`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:902-913`) only checks
`if !self.draft.can_wake(host) { return None; }` without checking `host.identity` (and `LaunchDraft::can_wake` at
`request.rs:406-411` does not check `host.identity` because plain-folder launches do not require it). As a result,
`checkout_folder_detail` (`view/checkout.rs:1266-1267`) draws "The folder is picked automatically when the host is
ready" (or "The folder is named after the session when the host is ready") even though `checkout_refusal` refuses the
launch with `UNVERIFIED_HOST`. Fix direction: Gate `checkout_allocation_notice` on the same verified-identity check as
`LaunchDraft::checkout_refusal` (or expose a shared `can_wake_checkout(&self, host: &HostView) -> bool` helper on
`LaunchDraft` used by both `request.rs` and `view/checkout.rs`).

Reviewer metadata (g10-wake-launch-state-and-requests/f6): suggested bucket: other (not highest: fail-closed UI/copy
inconsistency, Launch is still refused with UNVERIFIED_HOST); confidence: confirmed; possible cover: none (cross-group
overlap with g11 finding f5).

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:902` (g11-wake-launch-views-and-panes/f5):

In `crates/gui/farhelm-gui/src/launcher/request.rs:388-397`, `LaunchDraft::checkout_refusal` checks
`host.identity.as_deref().is_some_and(|id| !id.is_empty())` BEFORE `if self.can_wake(host)` and returns
`Some(UNVERIFIED_HOST)` when the host lacks an installation identity. However, `Launcher::checkout_allocation_notice`
(`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:902-913`) checks only `self.draft.can_wake(host)` without
checking `host.identity`. Consequently, `checkout_folder_detail` (`checkout.rs:1266-1267`) displays the wake-allocation
notice and `checkout_summary` (`checkout.rs:1320-1324`) shows `gh:owner/repo` on an unverified suspended/stopped host
even though a checkout cannot wake or launch on it. Fix direction: Require
`host.identity.as_deref().is_some_and(|id| !id.is_empty())` inside `checkout_allocation_notice` so it matches
`checkout_refusal`'s eligibility check.

Reviewer metadata (g11-wake-launch-views-and-panes/f5): suggested bucket: other (could not be in highest: misleading
status text only; Launch stays refused by checkout_refusal); confidence: confirmed; possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:1249-1296` (g24-launcher-compact-checkout/f4):

Verified against cross-group input from g11 (f5). In f249fdff, `Launcher::checkout_folder_detail`
(crates/gui/farhelm-gui/src/launcher/view/checkout.rs:1249-1296) delegates the wake state to
`self.checkout_allocation_notice(cx)` (checkout.rs:902-913), which checks `self.draft.can_wake(host)` without checking
that `host.identity` is present and non-empty. By contrast, `LaunchDraft::checkout_refusal`
(crates/gui/farhelm-gui/src/launcher/request.rs:388-397) checks
`host.identity.as_deref().is_some_and(|id| !id.is_empty())` before `self.can_wake(host)` and refuses with
`UNVERIFIED_HOST` ('The host installation is not verified; a checkout cannot be created'). Consequently, the new unified
Folder live region (`launcher-preview-wake`) announces 'The folder is picked automatically when the host is ready' (or
'named after the session when the host is ready') even though Launch cannot wake or create a checkout on that host. Note
also a follow-on interaction for the fix: if `checkout_allocation_notice` is fixed to require a non-empty
`host.identity`, `checkout_folder_detail` will fall through to `self.preview_answer(cx)` returning `None` (because
`preview_key` calls `checkout_host` -> `connected_host`, which also requires a connected, identified host) and display
`("pending", "Waiting for a current checkout preview")` indefinitely instead of indicating that the host is unverified
or unavailable. Fixing g11 f5 cleanly should therefore either handle unverified/unpreviewable hosts explicitly in
`checkout_folder_detail` or ensure the Folder status accurately reflects why no preview is pending.

Reviewer metadata (g24-launcher-compact-checkout/f4): suggested bucket: other (could not be in highest: checkout_refusal
still blocks Launch with UNVERIFIED_HOST, so no unverified checkout is created and no data loss or security boundary
bypass occurs); confidence: confirmed; possible cover: g11-wake-launch-views-and-panes f5 (same missing host.identity
check in checkout_allocation_notice).

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g10-wake-launch-state-and-requests/f6`,
`g11-wake-launch-views-and-panes/f5`, `g24-launcher-compact-checkout/f4`.
