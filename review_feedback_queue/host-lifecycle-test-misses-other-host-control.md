# The host lifecycle test misses the other host's displayed control

Reviewed commit: 504cd38da434644c7075e231cc82ef0a5a0b70a6

## TLDR

The test checks that a launch leaves another host available through a helper. It can pass even if the other host’s
actual menu is disabled.

## Details

`crates/gui/farhelm-gui/src/hosts/environment_tests.rs:1178` calls `host_lifecycle_refusal` directly for the other host.
The only menu opened belongs to the held host. Checking the other host’s menu would distinguish the intended UI
boundary.

Bucket: other. Confidence: verified from source at the pinned tip `66ad3aaa95ef03a2130e2d0194d58353912ae9da`.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`, finding `environment-controls/r23`.
