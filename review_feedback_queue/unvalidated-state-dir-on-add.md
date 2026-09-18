# Adding a host with a bad state-dir path permanently bricks the entry

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Registering a host with a bad state-directory path (empty or containing a NUL byte) permanently bricks the host entry:
every connection attempt fails with an opaque error, and there is no way to fix the path in place — the only recovery is
to delete the host and re-add it, which throws away its cached sessions and history.

## Details

Source: pre-pr-review-swarm, area stores, 2026-09-17. Confidence: definite. Coordinator verified by grep that no
`remote_state_dir_is_usable` predicate and no refusal variant exist anywhere in the crate.

`HelmStore::add_ssh_host` (`crates/farhelm-helm/src/store.rs:3303`) refuses an unusable `destination`
(`destination_is_usable`, store.rs:1261: non-empty, no leading `-`, no NUL) and an unusable `remote_farhelm`
(`remote_farhelm_is_usable`, store.rs:1268) before writing, but stores `remote_state_dir` with no validation at all. The
REST `add_host` path (`crates/farhelm-helm/src/hosts.rs:449`) passes `spec.ssh` and friends through verbatim, so nothing
upstream compensates.

Every later dial feeds the stored value into the ssh command line, so `Some("")` makes the remote proxy exit before the
handshake (`--state-dir ''`) and a NUL value fails the local ssh spawn opaquely on every attempt. Recovery is
remove-plus-re-add only: no setter for the column exists (coordinator grepped `SET remote_state_dir` — zero hits; only
destination and alias have setters), and re-add mints a new HostId, cascading away cached sessions and create history.

Suggested fix: add a `remote_state_dir_is_usable` predicate plus a refusal variant on `HostStoreError`, and check it
beside the `remote_farhelm` check before the write.
