# The New container comparison scene still uses the removed Runs on control

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

The container comparison scene targets the removed Runs on control and cannot reach the new Host menu route.

## Details

**other.** Source inspection.

docs/gpui/screenshot-scenes.json:765–777; crates/gui/farhelm-gui/src/launcher/view/host_picker.rs:327–347,641–675;
scripts/gui-screenshot-batch.py:507–522,538–557 at 1a53d9d35889d7dde96e31320cc1681dd013737d; introduced by
30441792a456f98227643624e65ae76620a5fb64

The comparison batch cannot reach the real-app New container capture using its scripted control, even on a host that
supports Podman. The scene waits for New container without opening Host and then clicks launcher-runs-on-New container.
The address-field change moved the choice into Host and made Runs on exclusive to templates
(launcher/view/placement.rs:202 at 30441792a456f98227643624e65ae76620a5fb64. SPEC.md:1010–1017 at the tip). The current
Host popover emits launcher-host-option-{name} and selects it through pick_host_choice. The comparison batch keeps this
live scene and withholds a screenshot when a step fails. The known-good control sequence is also visible in
goldens/launcher.rs:513–545, but that fake-helm golden is a separate scene (goldens/mod.rs:986–991). Correct the
comparison steps to open Host and choose New container, or explicitly retire the real-app scene if the golden replaces
it. The earlier mixed-case Destination wait is LV-15. This finding concerns the stale control even after that wait is
corrected. The reviewer suggested other (maintainer screenshot comparison). Confidence: confirmed for the missing
control path. Possible coverage: the sibling golden covers appearance but does not run this app scene.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-visual/LV-13`.
