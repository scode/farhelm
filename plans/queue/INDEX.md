# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `prepared-snapshots.md` — Farhelm-managed prepared snapshots per recipe (supervisor installed, no identity or secrets, stale after 24 h or an update, prepared lazily); new Tensorlake hosts start from them (after `recipe-bases.md`)
- [pending] `sandbox-sessions.md` — Launch a session in a named Tensorlake sandbox of its own from a recipe (launcher, templates, Clone), with explicit Suspend/Resume, Stop and suspend, Delete removing the sandbox, and agent operations refused (after `prepared-snapshots.md`)
- [in-flight 962dda] `macos-remote-hosts.md` — Set up, update and uninstall an Apple-silicon Mac as a remote host over SSH, with the supervisor as a per-user launchd job and no tmux on Mac hosts
- [in-flight 32f6b2] `own-host-sessions-without-asking.md` — A host permission, off by default, letting its sessions create, clone, rename, stop, restart and delete sessions on that same host without an approval card, with an "Always allow on <host> only" button on qualifying cards
