# Plans queue

One line per plan, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. This file is excluded from dprint so a line is never rewrapped.

- [in-flight f14e2e] `triage-clipboard-terminal-limit.md` — execute the 2026-10-02 highest-priority triage outcomes (a bound on the desktop window's pending clipboard writes, a hard size limit on terminal data from a supervisor, and removing the deferred filesystem-fallback queue item).
- [pending] `identity-report-wait-retry.md` — stop losing conversation-identity reports: no time limit on the supervisor's lock wait, Claude's sender check before it, a 30 s hook budget with retries across a supervisor restart.
- [pending] `host-dialogs-and-menu.md` — the host menu matches the session menu, and adding and removing a host each happen in a pop-up dialog with a "don't ask again" answer kept by the helm.
- [pending] `host-update-button.md` — an amber or red outlined update button on hosts that run an older Farhelm, in place of the `old version` / `needs update` word (after `host-dialogs-and-menu.md`)
- [pending] `claude-background-wait-status.md` — Claude reads as working, not idle, while its screen says it is waiting for background work to finish.
- [pending] `gh-clone-fresh-checkout.md` — Clone or Replace with into a fresh GitHub checkout uses the next free `repo-N` instead of refusing the copied title as taken.
- [pending] `stepped-animations.md` — looping indicators (the running pulse and its kin) step at most 10 times a second instead of redrawing every frame, and pause while the window is not active, with the rule written into the specs.
