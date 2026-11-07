# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `settings-credentials.md` — Settings → Credentials: named GitHub tokens, Claude Code and Codex subscription logins and endpoint tokens for Tensorlake recipes, delivered per host; GH_TOKEN with gh for private checkouts; "not encrypted on disk" warnings
- [pending] `macos-tunnel-exit.md` — Recipe tunnels end when the helm dies (a stdin pipe the sandbox side waits on) or goes silent (30 s heartbeat, 120 s timeout), verified on macOS CI and real sandboxes
- [in-flight cc0bf5] `row-hovers-beside-list.md` — Every hover in a session row (and the bell) opens beside the list, centred on its target, with a pointer notch and an accent ring on the hovered mark; the summary card too
- [pending] `cut-path-links.md` — A printed path that a program broke across two lines links to the whole path from both halves, specced as a deliberate exception to conservative recognition
- [pending] `instant-trash-delete.md` — Trash deletes answer at once and hosts remove the files in the background; the dialog closes on a confirmed bulk delete and the trash icon animates until the space is freed
- [pending] `stop-closes-tabs.md` — Stop closes the session's terminal tabs, asking first when tabs are open (skippable), with a precondition when it did not ask; Restart keeps tabs
- [pending] `no-gc-at-launch.md` — No automatic git cleanup on any git command a launch waits on; the repository cache consolidates in the background after every launch, spares on or off
