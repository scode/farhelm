# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [complete] `podman-sandbox.md` — Experimental containerized sessions on Linux: the agent runs in a rootless podman container that sees only its managed checkout, one API-proxy port on the host and an optional GitHub token, with terminal tabs inside the container
- [in-flight 8a7744] `menu-single-highlight.md` — Menus highlight one item at a time: hovering an item moves keyboard focus to it, so a click-opened menu no longer keeps its first item lit beside the hovered one (after `steel-sentence-case.md`)
