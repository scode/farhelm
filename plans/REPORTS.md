# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`template-number-shortcuts`](reports/template-number-shortcuts.report.md) landed 2026-10-14 in #2008, #2013: Bind templates to number keys; Cmd+digit in the Mac app, Ctrl+Shift+digit elsewhere, launches directly like quick launch
- [`intro-tour`](reports/intro-tour.report.md) landed 2026-10-14 in #2014, #2015: A welcome card on first open per helm, pointing at Get started and Send feedback; reopened from the ? menu
- [`ssh-2fa-help`](reports/ssh-2fa-help.report.md) landed 2026-10-14 in #2018: A ? menu dialog (and add-host link) showing the ssh commands that start a 2FA host's two shared connections by hand
