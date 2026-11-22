# The inline Resume test misses its disabled state

Reviewed commit: 504cd38da434644c7075e231cc82ef0a5a0b70a6

## TLDR

The inline Resume test can pass when the button looks and announces itself as enabled while its click handler refuses
the request.

## Details

`crates/gui/farhelm-gui/src/hosts/environment_tests.rs:1203` checks help text and the absence of a Resume command. Those
assertions leave the rendered enabled and accessibility states unobserved.

Bucket: other. Confidence: verified from source at the pinned tip `66ad3aaa95ef03a2130e2d0194d58353912ae9da`.

Review evidence retained under run `26a81958-b7bd-4dbd-a275-d09060967ae5`, finding `environment-controls/r25`.
