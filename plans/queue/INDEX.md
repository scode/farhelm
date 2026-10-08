# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.
- [in-flight f691ff] `new-button-shortcut-hint.md` — In the Mac desktop app, the New button's hover text names its Cmd+N shortcut.
- [in-flight 50554c] `template-launch-kind-rule.md` — One copy, in the shared protocol crate, of the rule that infers an old template's launcher tab and of the field groups it uses.
- [in-flight bdcc44] `repo-clone-cache.md` — Fresh GitHub checkouts go through a per-host repository cache in the supervisor's state directory, so only new objects cross the network; unused caches are removed after 30 days.
- [pending] `save-launcher-as-template.md` — "Save as template" in the New session dialog: name, checklist of the current choices with explicit ones pre-checked, then the new template opens in the Templates panel.
