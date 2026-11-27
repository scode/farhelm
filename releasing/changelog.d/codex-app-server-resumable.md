---
kind: fixed
---

Codex conversations started through Codex's shared background service, which a Codex in another terminal can attach
to, can now be resumed in Farhelm. Before, a Farhelm session that resumed one never offered Restart, even once nothing
else had the conversation open.

Farhelm now accepts the transcripts Codex marks with its app-server's default source (`vscode`), whatever client wrote
them. That widens which conversations Farhelm captures. A conversation that only the plain terminal Codex ever touched
was already resumable; this adds the ones that went through an app-server. Conversations Codex marks as coming from an
MCP server or of unknown origin are still not captured. Unverified, for the curator: the ChatGPT desktop app and the VS
Code extension probably use the same source, but which source the desktop app's own server stamps was not checked. If
the conversation is open in another app, Codex still shows its lock screen; forking from there is covered by the
separate Codex fork entry.
