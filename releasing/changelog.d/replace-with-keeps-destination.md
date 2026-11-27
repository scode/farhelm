---
kind: changed
---

Replace with now only replaces what runs in a session: the agent or command, model, effort, permissions, wrapper, prompt and title. It always stays on the source session's host and in its folder or managed checkout, which the launcher shows but no longer lets you change. Use Clone to start a session in a different folder, repository or host.

In Replace with, recent setups are shown disabled, since each one would set its own folder, and a template that names a different folder or a repository is refused. The launcher no longer offers a fresh clone in place of the source's managed checkout, so its warning about that is gone too. The plan's release note says the same in the maintainer's words; nothing departs from it.

The helm now enforces the same rule for every caller, not only the launcher: a Replace with request that names a different folder, or asks for a new managed checkout, is refused before anything is created or deleted. The desktop app never sends either, and the `farhelm` command line has no replace command, so this only changes anything for a script calling the helm's HTTP API directly; it needs no entry of its own.
