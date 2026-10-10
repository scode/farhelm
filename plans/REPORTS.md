# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`gui-text-safety`](reports/gui-text-safety.report.md) landed 2026-10-10 in #1811: pastes lose hidden end-of-paste markers, template summaries and identity labels show hidden characters, and templates refuse commands with control or invisible characters
- [`ui-interaction-fixes`](reports/ui-interaction-fixes.report.md) landed 2026-10-10 in #1812: menu arrow keys follow the selected item when items change, plain clicks stop re-copying old selections, and Replace on a failed session warns about the conversation
- [`helm-cli-fixes`](reports/helm-cli-fixes.report.md) landed 2026-10-10 in #1813: feedback and update/uninstall planning survive a closed page, a stopped host's connection cannot be orphaned, state-directory advice and ssh paths are right, and a Codex draft is not read as a question
- [`dev-tooling-fixes`](reports/dev-tooling-fixes.report.md) landed 2026-10-10 in #1815: the CentOS test keeps the user's global ssh settings global, the tmux build works on macOS bash 3.2, the plans queue and watcher, test-run recorder, cutover probe, changelog sweep and desktop smoke stop misreporting
