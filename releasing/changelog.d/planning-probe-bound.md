---
kind: fixed
---

Setting up, updating or removing a host no longer waits indefinitely when a program that ssh started on the machine
running Farhelm keeps the connection's output open after the command on the host has finished: Farhelm stops waiting
after a few seconds and reports the step as failed, so other hosts' setup, updates and removals are no longer held up.
Sending files to the host during setup or update can still wait that way.

This closes a possibility found by code reading; no SSH configuration that triggers it in practice has been
demonstrated. BUGS.md describes the remaining file-transfer case.
