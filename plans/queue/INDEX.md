# Plans queue

One line per plan that has not landed, in queue order, oldest first. `plans/AGENTS.md` describes the states; only `scripts/plans-queue.py` changes them, and planning PRs add new `[pending]` lines, at the end unless the maintainer places them elsewhere. A landed plan moves to `plans/REPORTS.md` until the maintainer has reviewed its report. This file is excluded from dprint so a line is never rewrapped.

- [landing 299857] `podman-sandbox.md` — Experimental containerized sessions on Linux: the agent runs in a rootless podman container that sees only its managed checkout, one API-proxy port on the host and an optional GitHub token, with terminal tabs inside the container
- [complete] `menu-single-highlight.md` — Menus highlight one item at a time: hovering an item moves keyboard focus to it, so a click-opened menu no longer keeps its first item lit beside the hovered one (after `steel-sentence-case.md`)
- [complete] `terminal-zoom-icons.md` — Terminal text size: magnifier zoom-out and zoom-in buttons with the current size in points between them, replacing "A−" and "A+"
- [complete] `host-update-discoverability.md` — Host updates you can't miss: an ↑ on the Hosts button, a card offering "Update all now" for optional updates, a banner for required ones, and a Settings switch for the helm to update remote hosts automatically
- [complete] `new-terminal-button.md` — The new-terminal control becomes a square icon-only "+" after a divider, so it no longer reads as another tab
- [pending] `host-short-names.md` — Hosts get a short name (alias, else the first hostname label), shown under each rail icon, beside it when the rail expands on hover, and on session rows
