# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`stop-closes-tabs`](reports/stop-closes-tabs.report.md) landed 2026-11-08 in #3132, #3133: Stop closes the session's terminal tabs, asking first when tabs are open (skippable), with a precondition when it did not ask; Restart keeps tabs
- [`agent-settings-compare-replace`](reports/agent-settings-compare-replace.report.md) landed 2026-11-08 in #3173: Tensorlake hosts: credential delivery never overwrites an agent settings file that changed after it was read
- [`tensorlake-name-ownership`](reports/tensorlake-name-ownership.report.md) landed 2026-11-08 in #3188, #3189, #3190: Tensorlake: adopt or delete a sandbox by name only when this helm asked for it, or spec that shared state is unsupported
