# Failing to delete the old embedded-payload cache blocks helm startup

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

If the helm's old embedded-payload cache contains a file it cannot delete, the whole helm refuses to start, taking the
web UI and access to every session with it.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F21 / COR-EMBEDDED-CLEANUP-BLOCKS-STARTUP`, tagged **possible**. Anchors and title: `provisioning/payloads.rs:723-735`,
`provisioning/payloads.rs:945`, `provisioning/service.rs:167`, `lib.rs:481-487` — Failing to delete the retired
embedded-payloads cache stops the helm from starting

Older helms cached payloads in `<state>/embedded-payloads`. At startup, `remove_leftover_embedded_payloads` deletes that
directory, which is pure disk-space housekeeping. Its errors are not treated that way: a failing `remove_dir_all`
(payloads.rs:723-735) returns `Err`, which propagates through `production_payloads_with_key` (payloads.rs:945),
`ProvisioningService::production` (service.rs:167) and `AppState::new` (lib.rs:481-487) and aborts helm startup. So does
an error from the check that guards against deleting an operator's `--payload-dir` when it points into that old cache
(that check canonicalizes paths and can fail). Realistic causes: a read-only subdirectory, or a root-owned file left by
running an old helm under `sudo`.

Nothing depends on the cleanup succeeding, yet its failure takes down the web UI and access to every session. Suggested
change: log a `warn!` naming the path and continue; skip the deletion when the aliasing check itself errors.

User-visible consequence: if the helm's old embedded-payload cache contains a file it cannot delete, the whole helm
refuses to start, taking the web UI and access to every session with it.
