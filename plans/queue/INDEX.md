# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 9aab32] `steel-sentence-case.md` — Steel redesign, last part: sentence case for every label in the app, SPEC.md, tests and the docs website's text (after `steel-sidebar.md`, `steel-session-chrome.md`, `steel-dialogs.md`)
- [pending] `muse-session-tracking.md` — Muse Code session tracking on the Codex and Claude model: hooks, conversation capture for Restart, and waiting/working status
- [pending] `trash-footer.md` — Trash dialog gets the shared header and pinned footer: Close and Delete all move to the footer with progress beside them, Delete all's confirmation sits above it (after `steel-sentence-case.md`)
