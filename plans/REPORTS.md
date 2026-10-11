# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`host-dialogs-redesign`](reports/host-dialogs-redesign.report.md) landed 2026-10-11 in #1865, #1866, #1867, #1868: redesign the add-host, setup confirmation, remove-host and uninstall dialogs to the approved mockup
- [`launcher-folder-shortcuts`](reports/launcher-folder-shortcuts.report.md) landed 2026-10-11 in #1874: replace the launcher's folder links with a dropdown on the folder field
- [`flickering-path-hover`](reports/flickering-path-hover.report.md) landed 2026-10-11 in #1879: keep a path hover steady while the terminal redraws under it
