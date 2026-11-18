# Managed Ubuntu preparation can time out while stopping bootstrap

Reviewed commit: 1a5b4a1ff6b1deb4551cfaf064176cbb6e174b9d

## TLDR

On some Linux Podman setups, a first Managed Ubuntu build can finish installing packages and still fail each time before
it can save an image.

## Details

Bucket: **high**. GUI migration final whole-tree review, R9 Sol Managed Ubuntu stop.

Source references: `crates/farhelm-helm/src/environment/podman/build.rs:41`,
`crates/farhelm-helm/src/environment/podman.rs:437, 460, 492, 509–554` at the reviewed commit (main
`crates/farhelm-helm/src/environment/podman.rs:464, 487, 519, 536–581`).

The bootstrap helper runs exec sleep infinity as container PID 1 under Podman's forced systemd mode. Capture first runs
podman stop --time -1 under an outer 60-second deadline. That mode may send SIGRTMIN+3, and a Linux PID namespace init
without a handler can ignore the stop. If it does, every cold build reaches the stop deadline instead of publishing its
image.

Sol rated this likely and noncritical. There is no independent verification. The command and timeout are in source.
Whether the pinned image and target Podman use that exact signal and PID 1 behavior needs a controlled runtime check.
High reflects the potential repeatable failure of the default base on an otherwise supported place. SPEC_impl.md's
Podman build section calls for stopping a helper before capture and does not accept failure on a healthy supported
Podman host.

Give bootstrap a PID 1 that handles the chosen stop signal or use a controlled stop path for this non-systemd helper.
Keep the clean-stop guarantee for real user-manager captures. Verify with the actual pinned base and an isolated Podman
place.
