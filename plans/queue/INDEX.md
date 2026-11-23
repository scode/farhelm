# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `codex-recipe-credentials-at-launch.md` — Supply Codex recipe API endpoint credentials at launch instead of writing ~/.codex/config.toml
- [complete] `explain-stale-launcher-choices.md` — Show inline stale-choice warning and keep query when a New Session launcher choice disappears
- [complete] `approval-card-program-wording.md` — Name the shell or main program accurately in Stop and Delete approval cards
- [complete] `wayland-clipboard-file-paste.md` — Read text/uri-list local files in Wayland clipboard paste with same-offer fallback
- [in-flight 9d8cd1] `toolkit-reduced-motion.md` — Follow GPUI's live reduced-motion setting everywhere and remove the desktop startup check
- [in-flight 783aae] `fallback-unattachable-remembered-tab.md` — Fall back to the main terminal when a remembered shell tab receives an explicit initial attach refusal
- [in-flight 224a28] `fix-muse-conversation-reporting.md` — Pass non-secret session identity args to Muse Code hooks across environment scrubbing
- [pending] `button-restyle.md` — Restyle buttons and button-like controls without outlines, with the larger session toolbar
