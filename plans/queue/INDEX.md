# Plans queue

One line per plan, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 296335] `update-while-running.md` — updating on a Mac while Farhelm runs is safe: each version in its own folder inside Farhelm.app, a tiny forwarder for sessions that outlive a restart, the app updated in place Chrome-style, and a restart that waits for the old Farhelm to finish (after `install-output-layout.md`, `desktop-internal-helm.md`)
- [pending] `local-update-disabled.md` — the hosts panel shows Update greyed out for this machine, pointing at the installer, so the helm's refusal and its stuck error are never reached (after `install-output-layout.md`, `update-while-running.md`)
- [in-flight 2bb563] `launch-representation.md` — launches become agent or command launches with named templates: profiles removed, no command-line parsing for YOLO, agent type or resume, Restart only ever resumes, legacy handling for existing sessions, and the agent CLI takes the launcher's fields as flags (after `identity-report-wait-retry.md`, `remove-identity-heuristics.md`)
