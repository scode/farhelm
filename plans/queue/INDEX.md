# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `launcher-typed-guarded.md` — typing guarded in the launcher selects Guarded like yolo, plus typed routes for the remaining permission and effort choices
- [landing 80441f] `json-download-open.md` — Cmd-click opens downloads of the types listed in a new setting (md, txt, json by default), with loud warnings for types not known to be safe
- [complete] `session-hover-text.md` — hovering a session row shows one styled summary card: host, checkout, agent, times
- [pending] `launcher-template-keys.md` — template shortcuts apply templates inside the open launcher, through one shared shortcut definition, and Cmd-J launches a template by name
- [pending] `repeat-quick-launch.md` — templates name sessions with a base name (numbered when taken) or a fixed name, so a named template can be quick-launched repeatedly into managed checkouts
- [in-flight cbc1f9] `host-uninstall-dialogs.md` — host setup, update and uninstall show their steps as a stepper in the dialog and in the row's progress, refusals open in dialogs, and the post-uninstall notice shrinks to one line
