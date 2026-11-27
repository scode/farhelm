---
kind: changed
---

The desktop app uses less CPU keeping its session list up to date when your hosts run many sessions.

Context for curation: this is a stack of changes from the desktop idle-CPU investigation
(`lore/2026-11-26-desktop-idle-cpu.md`). The embedded helm no longer rewrites every cached session row on each refresh
or copies the whole last-sent session list on each update it pushes to the window. Hosts also report the "last active"
time of sessions that keep working once a minute for all of them together, instead of one session at a time, so a
host full of busy agents no longer makes the app re-read everything about once a second. The visible cost: the "last
active" age shown on a session that is still working can lag by up to about two minutes instead of one; a session that
goes idle, starts waiting or ends shows its time at once. This needs the host updated, which an ordinary upgrade does.

A host whose connection to the app is too busy to take a "something changed" notice right away now keeps it and sends
it a moment later, instead of dropping it and leaving the change to the app's next periodic check. With notices no
longer lost, that periodic check of each host's session list now runs every thirty seconds instead of every three; the
waiting-session ntfy alert still goes out three seconds after a session starts waiting. Caveats: a host that stays
connected but stops answering now takes up to about a minute instead of about half a minute to show as unreachable, and
a passing failure to read a host's session list, with no change waiting behind it, can show for up to thirty seconds
instead of three.

For the curator: the last part bumps the protocol version to 95 (hosts update as part of an ordinary upgrade; per the
editorial guidance this needs no public remote-hosts entry, but it should be tested at release).
