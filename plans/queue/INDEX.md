# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `sticky-tmp-parent-refusal.md` — make the test recorder and deflake refuse another account's sticky parent directory, matching their docs, and close its triage outcome
- [landing a8ed8a] `brick-test-guard-oracles.md` — make two release brick-test harness tests prove the guards they name (CLI premise, each install prerequisite) and close their two triage outcomes
- [in-flight ae4618] `installer-expect-hyphen-value.md` — accept a hyphen-leading `--expect` value in `farhelm internal install` with a CLI-boundary test, and close its triage outcome
- [landing 3a3cb4] `installer-symlink-file-oracle.md` — make the installer's symlink test fail if file opens stop refusing links, and close its triage outcome
- [in-flight 3b8dda] `launcher-repository-search-loading.md` — show "Loading repositories…" instead of "no matches" while the launcher's repository search is pending, and close its triage outcome
- [in-flight 79e9b5] `launcher-folder-refusal-order.md` — classify a launcher folder refusal by the earliest refusal phrase so path text cannot misname it, and close its triage outcome
- [pending] `stop-wait-starting-launch-witness.md` — make the supervisor stop-wait test witness a launch still registering, and close its triage outcome
- [pending] `spare-builder-fifo-release.md` — keep the spare-builder test's named-pipe release from hanging when its reader exited, and close its triage outcome
