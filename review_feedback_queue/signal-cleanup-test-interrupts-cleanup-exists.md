# Signal-cleanup test interrupts before cleanup exists

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The signal-cleanup test interrupts before staging or traps exist.

## Details

F190 — **definite** — `scripts/test-install-sh.sh:2038` — Signal-cleanup test interrupts before cleanup exists

Its intended timing signals the installer during the initial checksum download, before staging is created or cleanup
traps are installed. Both final assertions can pass even with staging cleanup removed, because there is nothing to
clean. Establish staging and trap readiness, hold a later download at a controlled boundary, then signal and verify
removal.

## Evidence and triage context

- scripts/test-install-sh.sh:509 starts the installer without a supplied checksum file.
- scripts/test-install-sh.sh:362 delays every slow-path response by two seconds.
- scripts/install.sh:936 fetches checksums before creating staging at :976 and installing traps at :982.
- scripts/test-install-sh.sh:2038 waits half a second and :2044 accepts nonzero exit plus absent staging.
- scripts/test-install-sh.sh:2038 interrupts after half a second.
- scripts/test-install-sh.sh:362 delays checksum delivery for two seconds.
- scripts/install.sh:936 precedes staging creation and trap installation at :976 and :982.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No exact coverage identified; this is a concrete false-pass test execution.
- Fully covered by cli_installation_10_cor:p1:F2 retained in this batch.

Caveats:

- Scheduling delays can move the interruption point.
- No installer mutation or runtime test was performed.
- Scheduling can delay interruption into a later phase, but that does not remove the false-pass execution.
- No mutation test run.
- Scheduling can delay the test process enough to reach a later phase; that does not remove the demonstrated false-pass
  execution.
- No runtime tests or mutation tests were run.
- No mutation test or runtime reproduction performed.
- Preserve the concrete pre-staging false-pass explanation.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `cli_installation_10_cor:p1:F2`,
`cli_installation_10_sec:p1:F3`.

- `cli_installation_10_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: other.
- `cli_installation_10_sec:p1:F3`: confidence as filed: definite; suggested bucket as filed: other.
