# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`stop-closes-tabs`](reports/stop-closes-tabs.report.md) landed 2026-11-08 in #3132, #3133: Stop closes the session's terminal tabs, asking first when tabs are open (skippable), with a precondition when it did not ask; Restart keeps tabs
- [`agent-settings-compare-replace`](reports/agent-settings-compare-replace.report.md) landed 2026-11-08 in #3173: Tensorlake hosts: credential delivery never overwrites an agent settings file that changed after it was read
- [`prepared-snapshots`](reports/prepared-snapshots.report.md) landed 2026-11-08 in #3203: Farhelm-managed prepared snapshots per recipe (supervisor installed, no identity or secrets, stale after 24 h or an update, prepared lazily); new Tensorlake hosts start from them
- [`own-host-sessions-without-asking`](reports/own-host-sessions-without-asking.report.md) landed 2026-11-08 in #3205, #3206, #3208, #3211: A host permission, off by default, letting its sessions create, clone, rename, stop, restart and delete sessions on that same host without an approval card, with an "Always allow on <host> only" button on qualifying cards
- [`omp-npx-docs`](reports/omp-npx-docs.report.md) landed 2026-11-08 in #3207: The OMP docs page stops promising that npx launches are followed (where `sh` is dash, as on Debian and Ubuntu, they are not; elsewhere untested), and misleading code comments say the same
