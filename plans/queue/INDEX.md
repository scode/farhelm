# Plans queue

One line per plan, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. This file is excluded from dprint so a line is never rewrapped.

- [in-flight ae2f9a] `identity-report-wait-retry.md` — stop losing conversation-identity reports: no time limit on the supervisor's lock wait, Claude's sender check before it, a 30 s hook budget with retries across a supervisor restart.
- [approved] `host-dialogs-and-menu.md` — the host menu matches the session menu, and adding and removing a host each happen in a pop-up dialog with a "don't ask again" answer kept by the helm.
- [pending] `host-update-button.md` — an amber or red outlined update button on hosts that run an older Farhelm, in place of the `old version` / `needs update` word (after `host-dialogs-and-menu.md`)
- [approved] `gh-clone-fresh-checkout.md` — Clone or Replace with into a fresh GitHub checkout uses the next free `repo-N` instead of refusing the copied title as taken.
- [in review] `stepped-animations.md` — looping indicators (the running pulse and its kin) step at most 10 times a second instead of redrawing every frame, and pause while the window is not active, with the rule written into the specs.
- [in review] `triage-busy-host-refusal.md` — refuse Stop, Restart, Rename and other management requests with "try again" when a host is busy instead of freezing typing on it, and take the session list off the management limit.
- [in review] `triage-signin-recovery.md` — exempt the desktop app's own credentials from token rotation and the client cap so it never re-signs in, and let the browser lose an in-flight action's result across its sign-in prompt.
- [in review] `triage-opencode-model-names.md` — OpenCode accepts model names without the `opencode/` prefix, and typing a model name never switches the selected harness.
- [in review] `triage-boundary-checks.md` — five cheap defensive checks: session ids from hosts, unknown profile fields, the provisioning lock map, restart-with validation, host-label escape tokens.
- [pending] `triage-crash-replies-and-locks.md` — a crashed Stop or Restart answers with an error, the per-host write lock survives a worker restart, and two systemd findings leave the queue.
- [pending] `yolo-icon.md` — honest permission marks (slashed shield for YOLO, green shield for composer launches that ask, amber question mark otherwise), one shared YOLO classifier, OpenCode/OMP/Goose defaulting to YOLO like Pi, and the internal "sensitive host" names renamed to the user-facing ones.
- [in review] `attach-refusal-reason.md` — setting up or updating a host fails at once with the real refusal (protocol skew, identity) instead of waiting 30 s and saying "timed out", using a per-attempt sequence number to ignore the refusal the host held before.
- [in review] `drag-copy-hint.md` — a once-per-page notice when a plain drag in a mouse-capturing program copies nothing, pointing at the program's own copy command or Option/Shift-drag, with Codex's copy key when the session is Codex.
- [in review] `font-size-controls.md` — terminal text size via Cmd/Ctrl+Shift +/− and A−/A+ buttons at the right end of the tab strip, applied to every terminal and remembered per device.
- [in review] `unreleased-download-message.md` — a helm built from main says plainly that it is unreleased and has no release payloads to download, instead of "retry in a few minutes"; removes the obsolete provisioning-guard TODO entry.
