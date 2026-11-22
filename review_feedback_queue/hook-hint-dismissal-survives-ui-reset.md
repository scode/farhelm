# A hook hint hidden locally stays hidden after a UI connection reset

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A local hook hint dismissal can survive reconnect even though the UI reset is supposed to clear it.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d, SPEC.md:5491-5513 and 3098-3101;
crates/gui/farhelm-gui/src/main_view.rs:119-142, crates/gui/farhelm-gui/src/session_pane.rs:376-394,
crates/gui/farhelm-gui/src/session.rs:181-187, 204-218, 404-425, 534-548, and
crates/gui/farhelm-gui/src/session_pane/hook_band.rs:36-38, 59-75, 131-136, 163-176

After the app reconnects, the user still cannot see a hook setup hint that they hid locally before the connection broke.
The reset spec preserves the window, selected session and last terminal selection, and restores settings from the helm.
It says other local state starts over. Clicking × only adds the pair to a HashSet in the pane, distinct from the shared
preference saved by Don't show again. The window retains the same pane and HookBand, and while disconnected follow reads
no store and skips the only cleanup. When the rebuilt store contains the same still-eligible session, cleanup keeps that
pair. This needs no new launch, timing assumption, intermediate status or persisted dismissal. The reviewer suggested
other, missing discovery help until eligibility changes or the pane is recreated. Confidence: source confirms the reset
path and documented contract. Clear local hides when the UI session changes. This shares code with hook-hint-occurrence
but resetting on UI session changes alone will not fix coalesced restarts on a healthy connection.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`hook-contract/hook-hint-reset-contract`.
