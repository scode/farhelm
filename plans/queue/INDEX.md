# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 74e405] `template-placeholder-validation.md` — validate command placeholder quoting and joining when saving launch templates and close the matching review item
- [complete] `auto-update-staged-payloads.md` — skip automatic remote-host updates when locally staged unsigned payloads (`FARHELM_HELM_PAYLOAD_DIR`) are active and close the matching review item
- [in-flight 72121e] `harness-drift-tests.md` — add opt-in real-agent harness drift tests across all 10 supported harnesses (including live Antigravity conversation-report verification) with three-way verdicts and manual flow triggers
