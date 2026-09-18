# Interrupted installs orphan payload-sized hidden files on managed hosts

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

Every interrupted host install permanently leaks full-sized binary files onto that host — repeated failures from flaky
connections slowly fill the disk with invisible leftovers nothing ever cleans up.

## Details

Source: pre-pr-review-swarm, area provisioning, 2026-09-17. Confidence: definite (correctness-state-lifecycle p1).
Coordinator confirmed the nonce freshness, the own-temp-only cleanup, and the absence of any sweep.

Each install stages through `.{name}.farhelm-tmp-{probe_id}` in the destination directory, where `probe_id` is a fresh
UUID minted per probe (service.rs:422/705) and baked into the plan (plan.rs:306-311). The backend removes "exactly this
run's temporary path" (backend.rs:536) — before installing (597) and when the install errors (684). But a crash, kill,
or transport loss between temp creation and rename/cleanup leaves the file behind, and the next probe's new UUID means
no later run ever addresses that name. Nothing sweeps the pattern: no startup pass, no age rule, no documented manual
step. Each orphan is payload-sized (a farhelm or tmux binary, a unit file), hidden by its leading dot, in the live
install (`~/.local/lib/farhelm`) or unit directories.

Suggested fix: sweep `.*.farhelm-tmp-*` for the destinations being installed before each install (same transport,
best-effort, logged), or fall back to a stable per-destination temp name so a retry overwrites rather than orphans.
Either restores the "resume from content" convergence the module promises.
