# Managed-checkout wake tests omit Replace with despite parity claim

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

The test suite never exercises replacing an existing session with a fresh GitHub checkout on a suspended host—neither
via the form nor via Down+Enter on a retained recent checkout setup—leaving source-session locking across post-wake
preview, delete-guard preservation, preview-failure release behind Quick Switcher, and the missing fresh-clone
replacement warning uncovered.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/checkout_tests.rs:2287`.

Commit 3f6ea3b0cf6f55f82395569cf24be27e8e18d273 updated `docs/gpui/parity/launcher.md:1261-1267` to state that all 13
wake-launch cases from reference `e2e/tests/tensorlake-lifecycle.spec.ts` (including line 630, "managed Replace with
owns its source through wake and settles before Switcher return") are covered by `launcher::wake_tests`,
`launcher::checkout_tests`, and `templates::dispatch_tests`. However,
`crates/gui/farhelm-gui/src/launcher/wake_tests.rs:300-328` (`a_replacement_holds_its_source_through_a_wake_refusal`)
only exercises an existing-folder `Replace with` that fails during `POST /api/hosts/2/resume` (409), and the new
wake-checkout section in `crates/gui/farhelm-gui/src/launcher/checkout_tests.rs:2287-2711` contains only `Mode::New`
tests (`suspended_checkout` and `suspend_with_recents` both build `LaunchDraft::new`). None of the tests exercise
`Mode::ReplaceWith` with a managed checkout through host wake and post-wake preview, leaving four contracts untested:
(1) `wake_owns_source` (`crates/gui/farhelm-gui/src/launcher/launch.rs:269-273`, checked in `session_list/menu.rs:713`)
staying held across the post-wake `POST /api/github-checkout-preview` wait once the host reconnects (`Phase::Ready`),
(2) releasing `wake_owns_source` immediately when that preview fails even while the user has moved on to another modal
such as Quick Switcher (holding the launcher in `Waiting::User` behind `Notice::LaunchNotStarted`), (3) preserving the
drawn `SourceConsent` (`DeleteGuard`) captured at press time through `Wake.consent` -> `continue_after_wake` ->
`Armed.source` -> `POST /api/sessions/<id>/replace` if the source session changes while waking or previewing, and (4)
cross-checked with `g11` `l2` (`highest`): in `Mode::ReplaceWith` for a source session belonging to a managed checkout
on a suspended host, pressing Down+Enter on a retained managed-checkout recent setup (`suspend_with_recents` /
`recent_checkout_enter_wakes_the_verified_host_before_previewing`, `checkout_tests.rs:2611-2695`) calls `take_recent`
(switching from keeping the source's checkout to a fresh clone) and immediately calls `self.press(window, cx)`
(`search/start.rs:139-146`) in the same keystroke; because `can_wake(host)` bypasses `WAIT_FOR_PREVIEW`
(`request.rs:395-397`), the replacement wakes and submits without ever drawing `replacement_checkout_warning`
(`SPEC.md:1744-1748`, `2188-2191`). Covering `Mode::ReplaceWith` in `checkout_tests.rs` would catch both `f1` and `g11`
`l2`. Verification/fix direction: add `gpui::test` coverage in `crates/gui/farhelm-gui/src/launcher/checkout_tests.rs`
for managed-checkout `Replace with` on a suspended host (form launch through Resume + post-wake preview + Quick Switcher
preview-failure release, and Down+Enter on a retained recent checkout setup asserting `replacement_checkout_warning` is
shown before any wake/replace).

Reviewer metadata (g13-wake-checkout-launch-tests/f2): suggested bucket: other (cannot be in highest: test-coverage gap
confined to test suite and parity record; note that the unguarded Replace-with recent-Enter product path it missed is
tracked as highest in g11 l2); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g13-wake-checkout-launch-tests/f2`.
