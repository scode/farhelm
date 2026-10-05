# Preserving a foreign command can overwrite an occupied backup

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Preserving a foreign command can overwrite an occupied backup.

## Details

`F27 / COR-INSTALLER-BACKUP` — **possible** — `scripts/install.sh:1283` — Preserving a foreign command can overwrite an
occupied backup

When `~/.local/bin/farhelm` belongs to the user rather than the installer, installation moves it to a visible backup
name containing the current UTC timestamp. If that name already exists, the installer appends its process ID once, then
runs `mv` without checking whether this second name is also occupied. An existing regular file at the second name can be
overwritten. An existing directory can instead receive the command inside it, leaving the reported backup path pointing
at a directory rather than the preserved command.

This is the path intended to preserve user files, so it must also preserve files already occupying backup names.
Allocate a destination without allowing overwrite, retry on every occupied shape or refuse safely, and cover both
occupied candidate names with a fixture that controls the timestamp and process-ID naming.

Suggested bucket: highest

Possible cover: none. The earlier preservation decision in `TRIAGE_OUTCOMES.md:1994–2018` describes an exclusive
hard-link operation that refused collisions; it does not cover this unchecked rename fallback.

Caveats: Both candidate names must already be occupied. That narrow premise has not been reproduced, and this finding
makes no claim about how often such a collision occurs in ordinary use.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `cli_data p1`, `cli_edges p1`, `cli_systems p1`, `cli_trust p2`.

Possible cover recorded during collection: none identified.

Collection caveats: Narrow unverified premise both candidate names occupied; no ordinary collision frequency/repro.

## Filed reviewer metadata

- `cli_data p1`: confidence as filed: possible / likely. Material premise: both the timestamp-based kept name and its
  timestamp-plus-current-PID fallback already exist. Given that premise, plain mv can overwrite the second regular file;
  its destination is never checked or exclusively reserved. Suggested bucket as filed: highest.
- `cli_systems p1`: confidence as filed: `possible`; `likely`, conditional on both preservation names already existing.
  The unchecked overwrite is confirmed by code; no occurrence frequency is established. Suggested bucket as filed:
  `highest`.
- `cli_edges p1`: confidence as filed: definite/confirmed conditional on both generated backup names already existing as
  regular files. The rare premise is a collision at the timestamp-plus-PID fallback; no runtime reproduction was
  performed. Suggested bucket as filed: highest (user-file loss).
- `cli_trust p2`: confidence as filed: possible / likely conditional on both candidate preservation names already being
  occupied, especially the timestamp-plus-pid fallback. Overwrite behavior is confirmed from the unguarded mv
  destination; ordinary occurrence is unverified. Suggested bucket as filed: highest.
