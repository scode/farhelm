# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `e2e-oracle-and-cleanup-fixes.md` — 13 triage outcomes: end-to-end test oracles that can pass or fail wrongly, fixture cleanup on failure paths, and gating the e2e suite to Linux
- [in-flight 6376d5] `unit-test-oracle-fixes.md` — 13 triage outcomes: supervisor, helm and test-sweep unit tests that prove what they claim
- [in-flight 4ecd7e] `browser-oracle-and-fixture-fixes.md` — 12 triage outcomes: browser specs that can pass while their behavior is broken, or skip cleanup after a failure (one test-only PR)
- [in-flight c9a9fe] `dev-tooling-robustness-fixes.md` — 9 triage outcomes: small robustness fixes in deflake, the recorder, the release smoke check, the brick test, the preview script, README hero staging and a tmux test helper; each discarded instead if its fix is not small
- [pending] `ui-layout-and-escaping-fixes.md` — 12 triage outcomes: header and sidebar refresh, no unattended takeover under a build mismatch, read-state queue, escaped Rename and template names, layout at minimum widths, refresh-failure notice, approvals-only templates, template delete guard, permission-switch reset
- [pending] `helm-supervisor-small-fixes.md` — 6 triage outcomes: setup/uninstall exclusion on a fresh machine, lock-table pruning, helm shutdown retiring clients, a total template-catalog cap, Pi reporter guard, and no automatic update over a newer manual install
