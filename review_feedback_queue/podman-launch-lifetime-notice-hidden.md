# New container launches hide the disposable-file warning in Host help

Reviewed commit: 30441792a456f98227643624e65ae76620a5fb64

## TLDR

The launcher only explains in Host help that deleting the last session removes a new container and its files.

## Details

**other.** Source inspection.

30441792a456f98227643624e65ae76620a5fb64 crates/gui/farhelm-gui/src/launcher/view.rs:2606-2617;
crates/gui/farhelm-gui/src/environment_ui.rs:283-307; crates/gui/farhelm-gui/src/launcher/view/host_picker.rs:453-467

When choosing where to start work, users can miss that files in a new container disappear with its last session unless
they open Host help. At parent 82c5c23129a913833d0a343026870ccbbc9579e5 launcher/view.rs:2353-2358,2401 the normal form
displays this fact as a line. The address commit draws only launch_notice, which is None for Podman, and the Podman
warning is left in the Host tooltip. This persists at 1a53d9d35889d7dde96e31320cc1681dd013737d
launcher/view.rs:2451-2462 and environment_ui.rs:283-307. Restore a concise inline statement covering the choice of a
disposable workspace. The reviewer suggested other, confirmed loss of prelaunch discoverability. Unwarned deletion is
not established. At the pinned tip session_list/delete.rs:261-293 requires a question for an environment even with skip
enabled, and environment_ui.rs:602-605,643-647 names file loss in both deletion lanes, consistent with
SPEC.md:5270-5283.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`launcher-form/container-deletion-notice`, with `launcher-address/LA-05`.
