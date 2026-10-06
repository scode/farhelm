# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`mac-release-test`](reports/mac-release-test.report.md) landed 2026-10-06 in #1682, #1683: Agent-driven Mac release test, first slice: the updater's test-only "latest" override, the per-release Tart VM recipe, and the bring-up document for the agent on the Mac host.
- [`clone-into-fresh-checkout`](reports/clone-into-fresh-checkout.report.md) landed 2026-10-06 in #1684: Clone on a session in a GitHub checkout opens the launcher on a fresh `gh:` checkout of the same repository, named after the session plus `-clone` (first free `-clone-N` when taken); choosing a folder gives today's Clone; Replace with, Replace and `farhelm agent clone` unchanged; UI-only
- [`templates-dialog-overhaul`](reports/templates-dialog-overhaul.report.md) landed 2026-10-06 in #1678, #1687, #1688: Redesign the Templates panel (list beside editor, only the fields a template sets, choices that follow the agent type, undo for delete) and have new templates carry their launch kind.
