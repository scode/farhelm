# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [landing 6be09f] `resolve-stale-notifications.md` — A session's "never reported which conversation" and "Restart can no longer resume" notifications mark themselves resolved (greyed, counted as read) once Restart can resume again; a problem that comes back re-opens the same notification as new.
- [in-flight 53a649] `quick-switcher.md` — Cmd+K (macOS) / Ctrl+Shift+K opens a Slack-style switcher: fuzzy-find any host's session and jump to it, open New with the typed name, or with `tl:` open New with a template applied. (after `templates-dialog-overhaul.md`)
