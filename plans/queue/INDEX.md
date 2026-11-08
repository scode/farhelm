# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight ec23f9] `recipe-bases.md` — A Tensorlake recipe gets a base (Tensorlake Ubuntu, your image or your snapshot) that Farhelm always builds on with packages, Claude Code and Codex, checking the base's requirements first
- [pending] `prepared-snapshots.md` — Farhelm-managed prepared snapshots per recipe (supervisor installed, no identity or secrets, stale after 24 h or an update, prepared lazily); new Tensorlake hosts start from them (after `recipe-bases.md`)
- [pending] `sandbox-sessions.md` — Launch a session in a named Tensorlake sandbox of its own from a recipe (launcher, templates, Clone), with explicit Suspend/Resume, Stop and suspend, Delete removing the sandbox, and agent operations refused (after `prepared-snapshots.md`)
- [pending] `tensorlake-name-ownership.md` — Tensorlake: adopt or delete a sandbox by name only when this helm asked for it, or spec that shared state is unsupported
- [pending] `tensorlake-settings-prompts.md` — Tensorlake settings: bind the leftover Delete question, word the recipe Delete question honestly
- [complete] `openssh-state-path-refusal.md` — Tensorlake: refuse helm state paths containing characters older OpenSSH cannot carry
- [in-flight 243c4d] `agent-settings-compare-replace.md` — Tensorlake hosts: credential delivery never overwrites an agent settings file that changed after it was read
- [in-flight 46596c] `maintainer-tool-safety.md` — maintainer tools: agent-tui stop deletes nothing it cannot identify; fixture scrubber catches userinfo URLs
- [in-flight 59c150] `triage-misc-nov7.md` — host removal written durably, Custom-words edit cancels a pending launch, a spec clause, two Podman TODO lines, three discards
