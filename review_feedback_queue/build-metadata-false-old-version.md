# Build metadata can falsely mark an equally recent supervisor as old

Reviewed commit: f087e0b68aed3eb57d90f71b23ef9aa5499cb023

## TLDR

A host running the same release can incorrectly show an old-version warning when the helm or supervisor build includes
metadata.

## Details

Found by pre-pr-review-swarm run `20261002-0459-f087e0b6-2298` (entire repository; correctness and security only),
`F5 / COR-BUILD-AGE`, reviewer `correctness_state_lifecycle`, pass 1. Confidence: **definite**. Review disposition:
**would fix**. Queue priority at recording: **other**.

Anchor against the reviewed commit: `crates/farhelm-helm/src/hosts.rs:328`. Recorded from the completed review without
rechecking code after rebasing onto main.

The review did not supply separate `proposed_drop` or `possible_cover` fields. Documentary coverage at recording: No
matching existing queue item or prior decision was identified.

The “old version” advisory compares parsed versions with `<`. In the pinned SemVer library, that comparison includes
build metadata, even though SemVer precedence does not. A supervisor reporting `1.0.0` therefore appears older than a
helm reporting `1.0.0+local`; likewise, `1.0.0+aaa` appears older than `1.0.0+zzz`. Both pairs represent equal release
precedence. The current metadata test checks only the opposite direction, where the mistake does not produce an
old-version warning.

This affects the advisory display, not whether the host can connect. Use the library's metadata-ignoring comparison,
`peer.cmp_precedence(&ours).is_lt()`, to match both the helper's documented contract and SPEC_impl.md. Test metadata on
the helm side and on both sides so equal precedence remains equal regardless of comparison direction.

Additional original anchor: `crates/farhelm-helm/src/hosts.rs:1027` (the one-direction metadata test); dependency
version: semver 1.0.28.
