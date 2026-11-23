# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 442cce] `remove-own-setting-permission.md` — Remove "Agent's own setting" permission mode, treat unproven launches as YOLO, and offer guarded Replace
- [in-flight 523946] `codex-recipe-credentials-at-launch.md` — Supply Codex recipe API endpoint credentials at launch instead of writing ~/.codex/config.toml
- [in-flight 01ea2a] `stop-and-suspend-timeout-feedback.md` — Distinguish Stop and suspend's 20-second reply notice from supervisor timeouts in helm and desktop app
- [landing 584cf0] `tensorlake-lifecycle-sandbox-identity.md` — Refuse Tensorlake Suspend and Resume without a confirmed sandbox and persist recovered IDs under the provisioning lock
- [in-flight 72b066] `tensorlake-leftovers-discovery-failure.md` — Keep previously discovered Tensorlake leftovers visible alongside cleanup rows when discovery fails
- [pending] `explain-stale-launcher-choices.md` — Show inline stale-choice warning and keep query when a New Session launcher choice disappears
- [pending] `approval-card-program-wording.md` — Name the shell or main program accurately in Stop and Delete approval cards
- [pending] `wayland-clipboard-file-paste.md` — Read text/uri-list local files in Wayland clipboard paste with same-offer fallback
- [pending] `toolkit-reduced-motion.md` — Follow GPUI's live reduced-motion setting everywhere and remove the desktop startup check
- [pending] `fallback-unattachable-remembered-tab.md` — Fall back to the main terminal when a remembered shell tab receives an explicit initial attach refusal
- [pending] `fix-muse-conversation-reporting.md` — Pass non-secret session identity args to Muse Code hooks across environment scrubbing
- [pending] `button-restyle.md` — Restyle buttons and button-like controls without outlines, with the larger session toolbar
