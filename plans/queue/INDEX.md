# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 88ae36] `trash-and-host-settings-dialogs.md` — trash and host settings dialogs: margins for every dialog, pinned headers and footers, "delete all" at the top, per-host delete progress
- [landing d2c61e] `text-fields-no-spellcheck.md` — turn off spell checking and autocorrect in every text field
- [landing d2c61e] `post-clone-in-settings.md` — the all-hosts post-clone command in Settings
- [in-flight 79c576] `launcher-folder-shortcuts.md` — replace the launcher's folder links with a dropdown on the folder field
- [pending] `host-dialogs-redesign.md` — redesign the add-host, setup confirmation, remove-host and uninstall dialogs to the approved mockup (after `trash-and-host-settings-dialogs.md`)
