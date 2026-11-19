# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` (or, when the maintainer asks, `[held]`) lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 7a6f80] `tooltip-placement-audit.md` — Hover help placement standard across the app: menus beside the panel, sidebar and rail stacks beside their edge, top band and dialog buttons below, dialog lists beside, tall controls at the pointer
- [in-flight ab85ec] `docs-shot-desktop-fixes.md` — Settings → Hosts auto-update wording, template summary ellipsis, session row place id for the docs shot
- [pending] `settings-search.md` — Search field in Settings: section list narrows to matches, pane lists matches, jump to the setting (after `docs-shot-desktop-fixes.md`)
- [complete] `emacs-text-field-keys.md` — Emacs Control-key editing (Ctrl+F/B/N/P/A/E/D/H/K) in text fields on macOS and Linux
- [in-flight 078eaa] `stop-agent-podman-containers.md` — Stop agent- and tab-started Podman containers through Podman before the scope kill, with notices on failure
- [pending] `persist-environment-seal.md` — Remember a sealed environment across helm restarts so a failed deletion keeps Retry and a suspended sandbox gets destroyed
