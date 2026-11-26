---
kind: fixed
---

Farhelm now checks that a container host is still the same machine and account before running more of its container commands: container setup scripts, proxy reads, and the Containers list's leftover listing and Delete. If the host's SSH destination now reaches somewhere else, those commands stop with an explanation instead of acting on the other machine, and restoring the original destination makes the existing containers usable again.

This is a sanity check rather than a defense: changing `~/.ssh/config` to point a host somewhere else while Farhelm is using it is not supported. The check also no longer accepts a reply that merely repeats what Farhelm sent.
