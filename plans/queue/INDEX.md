# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [pending] `settings-lockout-scope.md` — keep other hosts' Settings usable during a one-click Update; block it only while a setup or uninstall confirmation may open
- [pending] `compact-rows-test.md` — update the compact-rows and OMP composer browser tests to the permission wording since the guarded mode
- [pending] `goose-named-restart-test.md` — supervisor test that a named Goose restart clears the captured conversation in memory and a definitive failure restores it
- [pending] `click-to-open-paths.md` — Cmd-click a terminal file path in the desktop app to save and open it (.md, .txt, .html only), plus the redesigned file hover
