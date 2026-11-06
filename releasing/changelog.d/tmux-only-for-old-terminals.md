---
kind: changed
---

A host whose sessions all use new terminals no longer runs a tmux server. Farhelm now uses tmux only for old terminals,
and the tmux server holding them stops by itself once the last one is closed or deleted, including a server an earlier
version left running with nothing in it.

Context for curation: before this change every supervisor started its private tmux server at startup and asked it about
every session, new terminals included. Now it only adopts a server that is already running (one holding old terminals)
and asks it only about sessions that have old terminals. A missing or too-old tmux no longer stops the supervisor from
starting; old terminals it cannot reach are reported as not running, and their server is left running untouched. Their
agents may still be running there, so Delete and Restart of those sessions are refused (with the reason) until a
supported tmux is installed or those agents are stopped; their status is recorded as exited meanwhile. Installing
Farhelm still requires a supported tmux, unchanged.
