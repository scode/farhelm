# Plan reports to review

One line per landed plan whose report the maintainer has not reviewed yet, oldest first. `plans/AGENTS.md` describes the review; only `scripts/plans-queue.py` changes this file. It is excluded from dprint so a line is never rewrapped.

- [`templates-dialog-close`](reports/templates-dialog-close.report.md) landed 2026-11-21 in #4345, #4346, #4347: let the Templates dialog close: no save question for an untouched template, and a leave question shown over the dialog (templates and the Settings recipe editor)
- [`agent-add-host-takeover-hint`](reports/agent-add-host-takeover-hint.report.md) landed 2026-11-21 in #4354, #4357: point users to Add host (instead of a non-existent Take over row in Hosts) when an agent asks to add a host managed by another Farhelm
- [`antigravity-workspace-trust`](reports/antigravity-workspace-trust.report.md) landed 2026-11-21 in #4349, #4350, #4356, #4362: add an off-by-default Antigravity workspace-trust switch (all launches, via per-launch `--settings`) and screen-reader trust detection for Waiting status and the discovery banner
- [`model-family-cleanup`](reports/model-family-cleanup.report.md) landed 2026-11-21 in #4353, #4363: standardize catalog model family names to versionless shorthand words, drop `family_word` from the wire, and shorten wrong-agent model refusals
- [`template-placeholder-validation`](reports/template-placeholder-validation.report.md) landed 2026-11-21 in #4355: validate command placeholder quoting and joining when saving launch templates and close the matching review item
- [`auto-update-staged-payloads`](reports/auto-update-staged-payloads.report.md) landed 2026-11-21 in #4358, #4366: skip automatic remote-host updates when locally staged unsigned payloads (`FARHELM_HELM_PAYLOAD_DIR`) are active and close the matching review item
