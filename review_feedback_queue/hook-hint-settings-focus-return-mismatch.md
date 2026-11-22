# Closing Settings from the new hook hint returns focus to the hint instead of the gear

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Settings opened from a hook hint returns focus to the hint even though the Settings contract names the gear.

## Details

**other.** Source inspection.

crates/gui/farhelm-gui/src/session_pane/hook_band.rs:131–150, settings.rs:242–255, 327–342, focus.rs:1694–1739 and
SPEC.md:2486–2491 at 1a53d9d35889d7dde96e31320cc1681dd013737d. New entry point in
bb31959c2f22c96bbbb72da2e60eee4a0b288a6b. Older trust entry point and return policy at settings.rs:239–252, 295–331 in
d04f63f48a2b4a8eecf7c5071b339198a1483896.

A keyboard user who opens Settings from a Grok or Antigravity hook hint and closes it with that hint still visible lands
back in the session hint. SPEC says closing Settings returns focus to the gear. The hook hint Settings button passes its
own persistent focus handle to open_at_hook_setup. The dialog records that handle as ReturnTo::Control, and the
coordinator gives it focus when it is still drawn. If it disappeared, the coordinator uses its usual opener/recovery
fallback rather than naming the gear. The older trust hint already used the same opener policy, so the reviewed
per-agent-page patch did not introduce it on that route. The later hook hint added a new route that inherits it. Give
Settings a separate return target where the contract requires the gear. The reviewer suggested other. Confidence
confirmed by source for an ordinary close while the hint is still drawn and the dialog owns focus. The mismatch changes
focus only and cannot reach highest on the inspected route. ,

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`agent-settings/agent-settings-hint-return-focus`.
