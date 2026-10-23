# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `sign-in-preservation-sweep.md` — Remove the code that keeps and resends failed GUI preference saves and keeps the browser app mounted across signing in again; signing in again in the browser reloads the page
- [in-flight 8edfd3] `tunnel-port-and-link-ranges.md` — Localhost tunnels always take an OS-chosen Mac port, and link opening treats IPv6 spellings of local IPv4 addresses as local (three 2026-10-22 triage outcomes, two PRs)
- [complete] `triage-docs-oct22.md` — Spec text for five accepted behaviours (personal tmux started from a session, Goose and Pi child reports, shared checkout roots) and two discards from the 2026-10-22 triage
- [in-flight 1a3754] `resume-target-fixes-oct22.md` — Restart with clears the old conversation when hooks are off, and Grok and OMP attribution refuse nested runs in unrecognised panes (2026-10-22 triage)
- [in-flight 388997] `checkout-ownership-oct22.md` — Five supervisor fixes: no-creation-time checkouts are released, restarted sessions survive stale create retries, attachments survive interrupted Delete, duplicate pane rows keep claims, rewound upstreams get a fresh clone
- [pending] `supervisor-lock-outlives-work.md` — The supervisor keeps its state-folder claim until its background work has ended, as the helm does (2026-10-22 triage)
- [pending] `uninstall-guards-oct22.md` — Uninstall refuses aliased and mount-crossing layouts, unchecked offline retries and control characters in kept paths; host planning bounds a stuck output read (2026-10-22 triage)
- [pending] `update-refuses-newer-host-schema.md` — Update refuses a host whose supervisor database is newer than the supervisor it would install (2026-10-22 triage)
- [pending] `session-view-fixes-oct22.md` — Five session-view and launcher fixes: displaced menus close, Replace with rows say replace, sidebar clickable under the outage card, no false "nothing started", terminal keeps off focus moved after selection
- [pending] `dialog-saves-oct22.md` — Five dialog fixes: trash delete ignores clicks right after its list moves, Templates rechecks names, unknown shortcut saves say so, the commands help names deleting, unedited Settings Save writes nothing (after `sign-in-preservation-sweep.md`)
- [pending] `build-and-tooling-oct22.md` — Linux tmux terminfo path, plans queue ignores fork PRs, CentOS test keeps the ssh config, rerun installer repairs a damaged version folder, devbuild verifies the tmux signature (2026-10-22 triage)
