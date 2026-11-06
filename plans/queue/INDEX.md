# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 6e398e] `settings-credentials.md` — Settings → Credentials: named GitHub tokens, Claude Code and Codex subscription logins and endpoint tokens for Tensorlake recipes, delivered per host; GH_TOKEN with gh for private checkouts; "not encrypted on disk" warnings
- [in-flight 90feb8] `macos-tunnel-exit.md` — Recipe tunnels end when the helm dies (a stdin pipe the sandbox side waits on) or goes silent (30 s heartbeat, 120 s timeout), verified on macOS CI and real sandboxes
- [pending] `one-press-suspended-checkout.md` — A fresh GitHub checkout on a suspended host takes one Launch press: the host resumes and the session starts in an automatically picked folder
