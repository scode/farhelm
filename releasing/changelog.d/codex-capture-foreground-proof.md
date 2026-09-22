---
kind: added
pr: 811
---

Codex conversation identity is only captured when the hook can prove it is running in the session's foreground, so a stale or foreign report can no longer attach itself to the wrong session. Reports from hook assets older than this release are refused rather than trusted, so re-run provisioning on hosts that were set up before it.

Written from the diff at curation-seeding time; the PR has no description. Verify the "re-run provisioning" advice before it ships.
