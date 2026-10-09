# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [complete] `repo-clone-cache.md` — Fresh GitHub checkouts go through a per-host repository cache in the supervisor's state directory, so only new objects cross the network; unused caches are removed after 30 days.
- [landing 6f282e] `save-launcher-as-template.md` — "Save as template" in the New session dialog: name, checklist of the current choices with explicit ones pre-checked, then the new template opens in the Templates panel.
- [in-flight 66795c] `row-marks-desktop-utf8.md` — The desktop app declares UTF-8 for its page and stylesheet, so a session row with a notification bell stops drawing garbled characters over its agent marks.
- [pending] `conversation-notice-hook-restart.md` — The "conversation not learned" notification says, per agent, when the agent normally reports and what to do; an Enter answering a dialog no longer starts its clock; a restarted supervisor keeps checking the sessions it picks up.
- [in-flight a19677] `upload-stall-same-session.md` — Payload uploads report the remote file's size on their own ssh session, so stall detection works on hosts that allow one session per connection.
