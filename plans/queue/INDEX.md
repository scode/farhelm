# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [landing e73753] `untrusted-text-escaping.md` — status badges and the sidebar's folder line show host text with hidden characters made visible, and the helm logs malformed or refused supervisor messages escaped
- [complete] `git-env-isolation.md` — an inherited GIT_DIR no longer steers checkout preparation or the discovery test fixtures into another repository
- [complete] `os-readback-fixes.md` — the service-file reader refuses escapes and section spellings systemd reads differently, and macOS reads large hook argument blocks whole
- [pending] `ui-interaction-fixes.md` — menu arrow keys follow the selected item when items change, plain clicks stop re-copying old selections, and Replace on a failed session warns about the conversation
- [complete] `harness-tooling-fixes.md` — quoted fixture paths, a per-run spawn-test workspace, deflake stop checks process start time, full hostname scrubbing, and release advice that never reuses a tag
- [in-flight 8501a7] `gui-text-safety.md` — pastes lose hidden end-of-paste markers, template summaries and identity labels show hidden characters, and templates refuse commands with control or invisible characters
- [pending] `setup-refusals.md` — setup refuses install paths containing `$`, service files of a type other than simple read as unrecognised, and uninstall's lock advice is shell-quoted (after `os-readback-fixes.md`)
- [in-flight 96039e] `ssh-config-atomic.md` — the CentOS test edits the user's ssh config by atomic rename, and a failed step never overwrites it
