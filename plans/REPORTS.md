# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`cli-permission-prompts`](reports/cli-permission-prompts.report.md) landed 2026-10-05 in #1605, #1606, #1607, #1608, #1609, #1610: the helm asks the user (non-modal card, "always allow from this host" switch) before carrying out any acting `farhelm` CLI verb, spawn included; agents cannot start YOLO or arbitrary-command launches on hosts that ask before YOLO; closes SPEC.md's cross-host create/clone exception; agent template writes behind the prompt
