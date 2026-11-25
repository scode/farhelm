# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 2b20d7] `triage-custom-agents-nov24.md` — execute the 2026-11-24 triage outcomes for registered custom agents, in grouped PRs
- [pending] `triage-settings-nov24.md` — execute the 2026-11-24 triage outcomes for the Settings dialog, in grouped PRs
- [landing c7327d] `triage-podman-nov24.md` — execute the 2026-11-24 triage outcomes for containers (Podman), in grouped PRs
- [in-flight a9674b] `triage-environments-nov24.md` — execute the 2026-11-24 triage outcomes for sandboxes, waking, suspend and Trash, in grouped PRs
- [pending] `sandbox-auto-trust-and-claude-onboarding.md` — Automatically suppress workspace trust prompts in per-session sandboxes/containers and skip Claude Code first-run setup on new recipe hosts
- [in-flight 9cb45f] `global-checkout-repo-history.md` — Replace per-host Managed checkout repository suggestions and host directory scanning with one helm-global repository history
- [complete] `hide-unconfigured-podman-recipe-status.md` — Hide unrelated unready Podman status noise on multi-provider recipe cards and disable Rebuild on unusable shown places
- [pending] `launcher-style-quick-dialogs.md` — Give the quick switcher (Cmd+K), the template list (Cmd+J) and Rename (Cmd+L) the session launcher's look, with its pieces shared as components
- [in-flight 36cd8d] `tab-close-activation-order.md` — Activate the previous terminal tab in open order when closing an active shell tab
- [pending] `agent-catalog-cli-and-drift-cleanups.md` — Remove GPT-5.6 from the model catalog, allow Goose named Restart capture, accept leading-hyphen `--effort`, and clean up OpenCode and instructions drift checks
- [pending] `tensorlake-setup-and-repo-cache-fixes.md` — Isolate repository-cache startup sweep errors, refuse Tensorlake setup reruns on suspended or deleting sandboxes, and assert setup never reads agent settings
- [pending] `tailscale-private-socket-dirs.md` — Allocate private OS-chosen Tailscale SSH socket directories persisted across helm restarts and retire host masters on retarget and Remove without globbing `/tmp`
