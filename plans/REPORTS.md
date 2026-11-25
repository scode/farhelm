# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`triage-custom-agents-nov24`](reports/triage-custom-agents-nov24.report.md) landed 2026-11-25 in #4810, #4817, #4820, #4824, #4827, #4830, #4834, #4835, #4838, #4846, #4853, #4890, #4893, #4899, #4903, #4906, #4912, #4915, #4919, #4920, #4924, #4930, #4932, #4935, #4941, #4946, #4950, #4952, #4969: execute the 2026-11-24 triage outcomes for registered custom agents, in grouped PRs
- [`agent-catalog-cli-and-drift-cleanups`](reports/agent-catalog-cli-and-drift-cleanups.report.md) landed 2026-11-25 in #4945, #4951, #4954, #4956, #4959: Remove GPT-5.6 from the model catalog, allow Goose named Restart capture, accept leading-hyphen `--effort`, and clean up OpenCode and instructions drift checks
- [`tailscale-private-socket-dirs`](reports/tailscale-private-socket-dirs.report.md) landed 2026-11-25 in #4957, #4962: Allocate private OS-chosen Tailscale SSH socket directories persisted across helm restarts and retire host masters on retarget and Remove without globbing `/tmp`
