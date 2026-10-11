# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`agent-managed-checkout`](reports/agent-managed-checkout.report.md) landed 2026-10-11 in #1886: let `farhelm agent create` start a session in a fresh managed checkout (`--gh`/`--github`)
