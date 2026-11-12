---
kind: none
---

The Farhelm inside a sandbox or container can now delete one of its sessions and, when no other session remains, close
itself to new sessions so that the sandbox or container can be destroyed. Nothing uses this yet: your Delete still
works as before until the change that makes Delete keep a sandbox or container while other sessions remain, whose entry
is in environment-agent-requests.md. The protocol bump it needs is the one that entry already mentions.
