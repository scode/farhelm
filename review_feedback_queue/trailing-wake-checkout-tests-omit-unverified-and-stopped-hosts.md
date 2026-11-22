# Wake-checkout tests cover only verified suspended sandboxes

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

All wake-checkout tests only run against a verified suspended sandbox, so they miss contradictory UI on an unverified
resumable host and stopped-container wake regressions during managed-checkout launches.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/checkout_tests.rs:2293`.

In `crates/gui/farhelm-gui/src/launcher/checkout_tests.rs:2293-2310` (`suspended_checkout`), `2611-2674`
(`suspend_with_recents`), and `crates/gui/farhelm-gui/src/templates/dispatch_tests.rs:600-649` (`begin_template_wake`),
every wake-checkout test reuses host 12 from `fixture_hosts(&[0, 11])` and explicitly asserts `host.identity.is_some()`
and `matches!(host.state, HostStateView::Suspended { .. })`. Two resumable host states that reach the wake-checkout code
paths are completely untested:

1. Unverified resumable host (`host.identity == None`, cross-checked with `g11` `f5`): `LaunchDraft::checkout_refusal`
   (`crates/gui/farhelm-gui/src/launcher/request.rs:388-397`) refuses Launch with `UNVERIFIED_HOST` before
   `can_wake(host)`, whereas `Launcher::checkout_allocation_notice`
   (`crates/gui/farhelm-gui/src/launcher/view/checkout.rs:902-913`) checks `self.draft.can_wake(host)` without checking
   `host.identity`, so the form still promises "The folder is picked automatically when the host is ready" (or "named
   after the session...") while refusing Launch. Because `suspended_checkout` and `pre_wake_traffic` only test
   `host.identity.is_some()`, this contradiction is untested.
2. Stopped Podman container (`HostStateView::Stopped`, cross-checked with `g10` `l1`): `LaunchDraft::can_wake` admits
   stopped containers alongside suspended sandboxes (`request.rs:406-411`), and `checkout_tests.rs` /
   `dispatch_tests.rs` never exercise `HostStateView::Stopped` with a managed checkout (including coalesced `Stopped`
   pushes after `CARRIES_ON` and failure wording). Verification/fix direction: add `checkout_tests.rs` cases for (a) an
   unverified suspended/stopped host asserting no allocation promise is shown and Launch refuses with `UNVERIFIED_HOST`,
   and (b) a stopped Podman container checkout wake through `CARRIES_ON` and `Stopped` pushes.

Reviewer metadata (g13-wake-checkout-launch-tests/f3): suggested bucket: other (cannot be in highest: test-coverage gap
confined to tests); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g13-wake-checkout-launch-tests/f3`.
