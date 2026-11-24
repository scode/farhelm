---
kind: fixed
---

Farhelm supplies recipe API endpoint credentials when it launches Codex, so changing an endpoint no longer rewrites
Codex's settings file. Restart a running Codex session to pick up a replacement.

Farhelm keeps the last delivered Claude and Codex launch credentials on the host if it cannot read either agent's
saved recipe credential. The host warning names the agent whose credential could not be read.

Caveats for curation: typing `codex` in a terminal tab no longer reaches the recipe endpoint once Farhelm has removed
its old settings entry; it uses that shell's own Codex settings. A token already set on the image's `farhelm` provider
can still be sent to the recipe's URL when the recipe selects no token.
