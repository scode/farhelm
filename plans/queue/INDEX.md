# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [landing 5dd12f] `sweep-on-timer.md` — the supervisor sweeps on its 2 s timer only, and stops re-reading settled stopped sessions
- [landing 5dd12f] `waiting-sound.md` — sounds when a session waits, an approval arrives, or (opt-in) a turn finishes, each switchable per device in Settings
- [landing 5dd12f] `terminal-file-download.md` — path-shaped text in a terminal is checked on hover and downloads from the session's host on click (100 MB limit; desktop saves to Downloads)
- [complete] `untrusted-text-escaping.md` — status badges and the sidebar's folder line show host text with hidden characters made visible, and the helm logs malformed or refused supervisor messages escaped
- [complete] `git-env-isolation.md` — an inherited GIT_DIR no longer steers checkout preparation or the discovery test fixtures into another repository
- [in-flight 8dafad] `os-readback-fixes.md` — the service-file reader refuses escapes and section spellings systemd reads differently, and macOS reads large hook argument blocks whole
- [pending] `ui-interaction-fixes.md` — menu arrow keys follow the selected item when items change, plain clicks stop re-copying old selections, and Replace on a failed session warns about the conversation
- [in-flight b5845d] `harness-tooling-fixes.md` — quoted fixture paths, a per-run spawn-test workspace, deflake stop checks process start time, full hostname scrubbing, and release advice that never reuses a tag
