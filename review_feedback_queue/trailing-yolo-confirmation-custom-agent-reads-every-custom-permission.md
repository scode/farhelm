# YOLO confirmation for a registered custom agent reads "every Custom permission choice" instead of naming the custom agent

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

When you launch or plain-replace a registered custom agent on a host that asks before YOLO launches, the confirmation
dialog explains the YOLO classification as "Farhelm treats every Custom permission choice as YOLO." Because "Custom" is
also the name of the Custom permission flags mode, this reads as if the launch used custom permission flags rather than
a registered custom agent.

## Details

**other.** Source inspection. Main reference `crates/gui/farhelm-gui/src/launcher/yolo.rs:73-78`.

In crates/gui/farhelm-gui/src/launcher/yolo.rs:40-42, YoloReason::of_launch checks
`!selection.harness.offers_non_yolo_mode()` and returns `YoloReason::OnlyMode(selection.harness)`. For a registered
custom agent, `selection.harness` is `LaunchHarness::Custom`, whose `offers_non_yolo_mode()` is false
(crates/farhelm-proto/src/launch.rs:407-414, 582-584). In yolo.rs:73-78, `YoloReason::sentence` formats
`OnlyMode(harness)` using `harness_label(harness)` (`"Custom"` from draft.rs:1538), producing
`"Farhelm treats every Custom permission choice as YOLO."` (and neither naming the registered custom agent nor
distinguishing a custom agent from custom permission flags). This interaction arose between commit 7b226839 (which
changed OnlyMode's wording to `"Farhelm treats every {name} permission choice as YOLO."`) and later custom-agent commits
(g49_custom_agents_gui_plumbing).

How to verify: Call `YoloReason::of_launch(Some(&selection)).sentence()` with `LaunchHarness::Custom` and
`permissions: Some(LaunchPermission::Yolo)` (or `None`).

Fix: Handle `LaunchHarness::Custom` explicitly in `YoloReason` (e.g. a dedicated variant or carrying the resolved
custom-agent label/wording such as "Farhelm treats every custom agent launch as YOLO.") so it does not interpolate
`"Custom"` into `"every {name} permission choice"`.

Reviewer metadata (g20_yolo_unset_gui_launcher/l2): suggested bucket: other — confusing wording in the YOLO confirmation
dialog for registered custom agents; the YOLO gate still blocks and requires explicit confirmation, so there is no
security bypass or data-loss risk.; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g20_yolo_unset_gui_launcher/l2`.
