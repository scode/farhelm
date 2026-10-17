# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 4e0ffc] `agent-delete-and-prompt.md` — `farhelm agent delete` (approved, no self-delete), and an initial prompt at creation from the CLI, the launcher and templates
- [pending] `template-name-escape.md` — Template editor: an older template's unsafe session name shows escaped and read-only, like model, folder and commands
- [complete] `calm-session-unavailable.md` — Session pane: one calm "session unavailable" card instead of red and amber error text whenever the supervisor is out of reach
- [pending] `replace-retry-identity.md` — Replace retries: one retry identity per source, and a fresh attempt after a definite refusal (2 outcomes, each gated)
- [landing 9b4d95] `triage-spec-rules-oct16.md` — Spec rules from the 2026-10-16 triage: Claude lock trade-off, stale views in a second GUI, trash scale (3 outcomes, each gated)
- [in-flight 133e84] `claude-trust-startup-no-wait.md` — Supervisor startup no longer waits on Claude's settings lock for leftover trust cleanup (1 outcome, gated)
- [in-flight 0ca962] `bare-cache-explicit-git-dir.md` — Fresh GitHub checkouts work on hosts with Git's explicit bare-repository policy (1 outcome, gated)
