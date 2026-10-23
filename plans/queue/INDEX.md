# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 868144] `duplicate-uninstall-retry-advice.md` — Once the other entry reaching the same machine is removed, the duplicate's uninstall refusal says to press Retry and uninstall through this entry, instead of naming the removed entry.
- [in-flight ab8cfe] `sign-in-preservation-sweep.md` — Remove the code that keeps and resends failed GUI preference saves and keeps the browser app mounted across signing in again; signing in again in the browser reloads the page
