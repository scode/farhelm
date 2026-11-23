---
kind: fixed
---

Farhelm skips automatic Claude Code trust in your home directory and every directory above it. This keeps a session
started in a parent directory from granting trust to everything inside your home.

Caveat for curation: v0.44.0 and v0.44.1 could already have marked such a directory (for example `/home` or `/Users`)
trusted in Claude's settings file, and this release does not remove that entry; it stays until you remove it. Farhelm
also skips the edit, and says why in the session's terminal, when it cannot resolve your home directory on the session's
host.
