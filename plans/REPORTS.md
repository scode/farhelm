# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`ordered-gui-helm-requests`](reports/ordered-gui-helm-requests.report.md) landed 2026-10-15 in #2062: The GUI holds a request back only until its own earlier configuration saves are answered, fixing two save/read races
- [`claude-trust-prompt`](reports/claude-trust-prompt.report.md) landed 2026-10-15 in #2070: Opt-in setting to pre-trust managed checkouts in Claude's config, with a hint band when Claude asks
