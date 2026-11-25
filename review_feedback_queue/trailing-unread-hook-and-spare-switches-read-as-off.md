# Unread hook setup and spare-checkout switches read as off to screen readers

Reviewed commit: 7612b9a95f1dfd4432db78951c6be37f23ab7f2d

## TLDR

While Settings has not yet read (or failed to read) whether an agent's "Set up conversation hooks on all hosts" switch
is on, a screen reader announces it as an ordinary switch that is off. A user relying on the announcement can conclude
Farhelm is not installing hooks when it may be. The same applies to the Checkouts page's spare-checkout switch. #4905
fixed exactly this for the workspace trust switches only.

## Details

7612b9a9 added `Checkbox::value_unknown` (`crates/gui/farhelm-gui/src/components/checkbox.rs:115-120` and `282-306` at
the reviewed commit) because "Some platform adapters infer an off value from a switch role alone", and switched only
`agent_trust.rs:465` from `.disabled(...)` to `.value_unknown(...)`. At the tip,
`crates/gui/farhelm-gui/src/settings/hook_settings.rs:52` and `crates/gui/farhelm-gui/src/settings/checkouts.rs:94`
still use `.disabled(!...available())`. Both are `Configured` switches whose `shown()` is a placeholder false while
`Reading` or `Unreadable` (`configured.rs:103-116`). `disabled` changes nothing in the accessibility node: the switch
still sets `role(Switch)` and `aria_toggled(Toggled::False)`, and vendored GPUI has no disabled property. The reviewer
checked the locked `accesskit_macos` 0.26.3: it reports the switch enabled (`!node.is_disabled()`,
`src/node.rs:633-635`) with AXValue false (`src/node.rs:323-326`), so an unread switch is exposed exactly like an
enabled, saved-off one.

SPEC.md "Keyboard focus" (added by #4864): "Consent, warning and status text in the desktop app must be available to
assistive technology". SPEC.md trust hints: "Failed settings reads never count as an off switch".

Fix: use `.value_unknown(!switch.state.available())` in both places (or make that the only way `Configured` switches
render while unknown), and extend the settings test #4905 added for the trust switch to these two.

Suggested bucket: `other` (misleading accessibility state on a setting that is still unknown; the switch cannot be
toggled while unknown). It would move up if the hook switch's read is shown to commonly stay pending. Confidence:
confirmed from source through the GUI and the macOS adapter. Related but separate queue item:
`hook-settings-toggle-uses-unshown-state.md` (Space saving the hook opt-in from unshown state), planned in
`plans/queue/triage-settings-nov24.md`; that fix does not change the unknown-value announcement.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g02-trust-a11y-settings-approvals` record `f1`.
