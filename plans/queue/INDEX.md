# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [complete] `repo-clone-cache.md` — Fresh GitHub checkouts go through a per-host repository cache in the supervisor's state directory, so only new objects cross the network; unused caches are removed after 30 days.
- [complete] `row-marks-desktop-utf8.md` — The desktop app declares UTF-8 for its page and stylesheet, so a session row with a notification bell stops drawing garbled characters over its agent marks.
- [complete] `conversation-notice-hook-restart.md` — The "conversation not learned" notification says, per agent, when the agent normally reports and what to do; an Enter answering a dialog no longer starts its clock; a restarted supervisor keeps checking the sessions it picks up.
- [complete] `upload-stall-same-session.md` — Payload uploads report the remote file's size on their own ssh session, so stall detection works on hosts that allow one session per connection.
- [in-flight 73e177] `version-hover-text.md` — Hovering the version readout says in plain words which Farhelm is running and, in the Mac app, that it turns red once an update is installed and can then be selected to restart into it.
- [in-flight 34eb07] `enter-launches-anywhere.md` — Enter on a choice in the New session and Restart with dialogs chooses it and launches or restarts; a launch that cannot happen says why next to the Launch button. (after `save-launcher-as-template.md`)
- [in-flight 32198a] `drag-copy-notice.md` — The "drag copied nothing" notice appears just above the pointer, stays up to 30 seconds with a dismiss button, and shows on every such drag.
- [in-flight 311a23] `preview-lock-identity.md` — The docs preview script leaves alone a process that started after the stale lock naming it was written.
