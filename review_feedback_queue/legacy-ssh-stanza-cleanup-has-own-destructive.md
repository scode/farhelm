# Legacy SSH stanza cleanup has its own destructive publication window

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Legacy SSH cleanup has a separate configuration-loss window.

## Details

F73 — **definite** — `scripts/test-provision-centos.sh:341` — Legacy SSH stanza cleanup has its own destructive
publication window

Before the test starts, legacy-marker cleanup rewrites the real SSH configuration through a truncating copy. If
publication is interrupted, unrelated aliases and settings can be left empty or partial with no restoration path. This
is independently reachable from ordinary stanza removal. Use an atomic publication mechanism for the legacy rewrite and
retain enough recovery material to preserve the user's original content on failure.

## Evidence and triage context

- scripts/test-provision-centos.sh:333 detects the legacy marker; :336–340 generates scrubbed content; :341 copies
  through a truncating redirect to the real config. Exit cleanup at :178 scrubs the destination rather than restoring an
  original, then :183 deletes scratch.
- scripts/test-provision-centos.sh:333-340 detects the legacy marker and generates a scrubbed replacement.
- scripts/test-provision-centos.sh:341 truncates the real SSH configuration before copying.
- scripts/test-provision-centos.sh:177-183 provides no restoration of the pre-migration contents.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2030–2031 excludes ordinary healthy-filesystem cancellation from the filesystem-failure acceptance.
- SPEC.md:2021-2031 excludes ordinary cancellation from the filesystem allowance. FILTER.md:36-42 excludes data loss. No
  exact coverage record was found.

Caveats:

- Requires a legacy block and interruption during its rewrite. Independently editable from the ordinary removal site.
- Requires the legacy marker and interruption during replacement.
- No runtime reproduction.
- The regular block-removal helper does not own this separate write site.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `automation_website_10_cor:p1:F3`,
`automation_website_10_sec:p1:F5`.

- `automation_website_10_cor:p1:F3`: confidence as filed: definite; suggested bucket as filed: highest.
- `automation_website_10_sec:p1:F5`: confidence as filed: definite; suggested bucket as filed: highest.
