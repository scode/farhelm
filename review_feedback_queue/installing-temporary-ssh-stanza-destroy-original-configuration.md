# Installing the temporary SSH stanza can destroy the original configuration

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Installing a test SSH stanza can leave the user's configuration incomplete.

## Details

F74 — **definite** — `scripts/test-provision-centos.sh:381` — Installing the temporary SSH stanza can destroy the
original configuration

The harness prepares a complete scratch replacement but publishes it by truncating and copying into the live SSH
configuration. Preparing the source first does not make that final copy interruption-safe: failure can leave unrelated
settings empty or partial. Publish atomically with appropriate handling of symlinks and permissions, and preserve
recoverable original content when replacement fails.

## Evidence and triage context

- scripts/test-provision-centos.sh:366–380 prepares the combined configuration. Line 381 truncates the real config
  before cat completes. Cleanup at :178 reads that possibly damaged destination, and :183 removes the run directory
  containing ssh-config.new.
- scripts/test-provision-centos.sh:366-380 prepares a complete replacement containing the original configuration.
- scripts/test-provision-centos.sh:381 opens the live configuration with truncating redirection before cat copies that
  replacement.
- scripts/test-provision-centos.sh:177-183 cleans the currently damaged file and removes the run directory rather than
  restoring ssh-config.new.
- scripts/test-provision-centos.sh:186 installs cleanup as the EXIT trap.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2021–2031 does not accept cancellation-induced loss on a healthy filesystem.
- SPEC.md:2021-2031 accepts filesystem errors and hangs but expressly excludes ordinary cancellation on a healthy
  filesystem. FILTER.md:36-42 excludes user-data loss.

Caveats:

- Narrow interruption window; separate setup publication site from findings F2 and F3.
- Requires interruption during the copy; no reproduction was performed.
- Whether scratch evidence is subsequently deleted depends on which processes receive the interruption and whether
  teardown completes.
- The live configuration is vulnerable even when a recoverable scratch copy survives.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_10_cor:p1:F4`,
`automation_website_10_sec:p1:F3`.

- `automation_website_10_cor:p1:F4`: confidence as filed: definite; suggested bucket as filed: highest.
- `automation_website_10_sec:p1:F3`: confidence as filed: definite; suggested bucket as filed: highest.
