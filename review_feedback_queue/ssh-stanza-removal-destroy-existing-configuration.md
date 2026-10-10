# SSH stanza removal can destroy the existing configuration

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Removing a temporary SSH stanza can destroy unrelated configuration.

## Details

F72 — **definite** — `scripts/test-provision-centos.sh:163` — SSH stanza removal can destroy the existing configuration

The stanza-removal path truncates the real SSH configuration before its replacement copy completes. Interruption or copy
failure can leave an empty or partial file, and generation failures are also suppressed rather than reliably stopping
publication. Unrelated aliases and settings can be lost. Guard generation explicitly and publish atomically, preserving
symlink-target behavior, permissions, and recovery material.

## Evidence and triage context

- scripts/test-provision-centos.sh:155–159 generates scratch content; :163 opens the real config for truncating output
  before cat completes. Cleanup invokes the function through || true at :178, so an awk failure need not stop the copy,
  then removes the scratch directory at :183. The helper is also called by stale-block cleanup at :324.
- scripts/test-provision-centos.sh:153-159 creates a scrubbed temporary copy.
- scripts/test-provision-centos.sh:160-163 intentionally writes through the existing live file using truncating
  redirection.
- scripts/test-provision-centos.sh:177-183 invokes removal during teardown and then deletes the scratch directory.
- scripts/test-provision-centos.sh:314-324 also invokes this helper during stale-block cleanup.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2021–2031 accepts local filesystem errors but expressly excludes ordinary cancellation on a healthy
  filesystem. FILTER.md:36–42 excludes data loss.
- SPEC.md:2021-2031 does not accept ordinary cancellation damage. FILTER.md:36-42 excludes user-data loss. No matching
  Planned item, queue item, BUGS entry or ledger decision was found.

Caveats:

- Requires interruption during publication or a failed generation in the suppressed-errexit context. No loss was
  reproduced.
- No runtime reproduction.
- Scratch deletion is conditional on teardown continuing after interruption.
- Fixing stanza installation alone would leave this write site vulnerable.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_10_cor:p1:F2`,
`automation_website_10_sec:p1:F4`.

- `automation_website_10_cor:p1:F2`: confidence as filed: definite; suggested bucket as filed: highest.
- `automation_website_10_sec:p1:F4`: confidence as filed: definite; suggested bucket as filed: highest.
