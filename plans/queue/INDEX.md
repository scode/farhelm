# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [landing 0d9684] `preview-lock-identity.md` — The docs preview script leaves alone a process that started after the stale lock naming it was written.
- [landing 76a77f] `transcript-reads-on-need.md` — The supervisor stops re-reading Codex and Grok transcripts on every pass (a later Codex hook confirms a `/clear`, Restart catches a deleted file) and drops the other per-pass overhead: empty report-folder renames, uncached statements, no-op notification writes. (after `conversation-notice-hook-restart.md`)
