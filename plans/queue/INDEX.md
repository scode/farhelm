# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 4e8f31] `antigravity-workspace-trust.md` — add an off-by-default Antigravity workspace-trust switch (all launches, via per-launch `--settings`) and screen-reader trust detection for Waiting status and the discovery banner
- [in-flight 2a27f9] `model-family-cleanup.md` — standardize catalog model family names to versionless shorthand words, drop `family_word` from the wire, and shorten wrong-agent model refusals
- [complete] `agent-add-host-takeover-hint.md` — point users to Add host (instead of a non-existent Take over row in Hosts) when an agent asks to add a host managed by another Farhelm
- [in-flight cddbf5] `template-placeholder-validation.md` — validate command placeholder quoting and joining when saving launch templates and close the matching review item
- [pending] `auto-update-staged-payloads.md` — skip automatic remote-host updates when locally staged unsigned payloads (`FARHELM_HELM_PAYLOAD_DIR`) are active and close the matching review item
- [pending] `harness-drift-tests.md` — add opt-in real-agent harness drift tests across all 10 supported harnesses (including live Antigravity conversation-report verification) with three-way verdicts and manual flow triggers
