# Crash-orphaned extraction staging files are never pruned

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

If the helm crashes while unpacking an install binary, the half-unpacked temp file stays in its cache forever — repeated
crashes grow the cache without bound and without any warning.

## Details

Source: pre-pr-review-swarm, area provisioning, 2026-09-17. Confidence: definite (correctness-state-lifecycle p1).
Coordinator confirmed the staging/persist split and the prune glob. Sibling of
`orphaned-install-temps-on-managed-hosts.md`, on the helm's own side.

The directory payload source stages extractions through `tempfile::NamedTempFile` (payloads.rs:450, 488) and publishes
by persist/rename (471, 503). In-process failures self-clean via `Drop` — but a crash (SIGKILL, power loss) between
stage and persist/drop orphans the temp, and `prune_stale_generations` only matches `<asset>.*.bin` snapshots (216-242),
never `.tmp*` staging names. The download source's startup housekeeping sweeps its own staging pattern; the directory
source has no equivalent.

Suggested fix: extend the prune glob to the staging pattern, or add a startup sweep of orphaned staging temps in the
directory source's cache, mirroring the download source's housekeeping.
