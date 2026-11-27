---
kind: changed
---

The desktop app uses less CPU keeping its session list up to date when your hosts run many sessions.

Context for curation: this is the first of a stack of changes from the desktop idle-CPU investigation
(`lore/2026-11-26-desktop-idle-cpu.md`). This part stops the embedded helm from rewriting every cached session row on
each refresh and from copying the whole last-sent session list on each update it pushes to the window; later PRs in the
same stack extend this entry.
