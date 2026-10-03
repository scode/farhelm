# Plans queue

One line per plan, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. This file is excluded from dprint so a line is never rewrapped.

- [in review] `triage-clipboard-terminal-limit.md` — execute the 2026-10-02 highest-priority triage outcomes (a bound on the desktop window's pending clipboard writes, a hard size limit on terminal data from a supervisor, and removing the deferred filesystem-fallback queue item).
- [in-flight ae2f9a] `identity-report-wait-retry.md` — stop losing conversation-identity reports: no time limit on the supervisor's lock wait, Claude's sender check before it, a 30 s hook budget with retries across a supervisor restart.
- [in-flight 7c97fd] `host-dialogs-and-menu.md` — the host menu matches the session menu, and adding and removing a host each happen in a pop-up dialog with a "don't ask again" answer kept by the helm.
- [pending] `host-update-button.md` — an amber or red outlined update button on hosts that run an older Farhelm, in place of the `old version` / `needs update` word (after `host-dialogs-and-menu.md`)
- [in-flight 72b35b] `claude-background-wait-status.md` — Claude reads as working, not idle, while its screen says it is waiting for background work to finish.
- [in-flight bb108d] `gh-clone-fresh-checkout.md` — Clone or Replace with into a fresh GitHub checkout uses the next free `repo-N` instead of refusing the copied title as taken.
- [pending] `stepped-animations.md` — looping indicators (the running pulse and its kin) step at most 10 times a second instead of redrawing every frame, and pause while the window is not active, with the rule written into the specs.
- [pending] `triage-busy-host-refusal.md` — refuse Stop, Restart, Rename and other management requests with "try again" when a host is busy instead of freezing typing on it, and take the session list off the management limit.
- [pending] `triage-signin-recovery.md` — exempt the desktop app's own credentials from token rotation and the client cap so it never re-signs in, and let the browser lose an in-flight action's result across its sign-in prompt.
- [pending] `triage-opencode-model-names.md` — OpenCode accepts model names without the `opencode/` prefix, and typing a model name never switches the selected harness.
- [pending] `triage-boundary-checks.md` — five cheap defensive checks: session ids from hosts, unknown profile fields, the provisioning lock map, restart-with validation, host-label escape tokens.
- [pending] `triage-crash-replies-and-locks.md` — a crashed Stop or Restart answers with an error, the per-host write lock survives a worker restart, and two systemd findings leave the queue.
