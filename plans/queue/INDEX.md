# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [in-flight 76b24d] `podman-rigor-todo.md` — containerized sessions: move eight review findings into one near-term Podman rigor TODO entry
- [in-flight af0397] `unlikely-races-nov4.md` — terminals: fix four very unlikely races where straightforward, accept the rest in FILTER.md
- [in-flight 03f5f7] `durability-revocations.md` — power loss: write host permission revocations durably, key retries by host entry, and spec the accepted losses
- [pending] `printed-link-mismatch.md` — printed links: no lasting mismatch between the shown and opened address; spec the hover warning
- [pending] `delete-unreadable-terminals.md` — Delete: keep warning about and reaping work after a terminal stops responding
- [pending] `terminal-text-drops.md` — new terminals: state the invisible-formatting drop, print Kirat Rai vowel signs again
- [pending] `triage-misc-nov4.md` — template card shows the container, escape host text in helm logs, paste-after-gap clause, one discard
