---
kind: fixed
---

Uninstalling Farhelm from a remote host no longer fails with "the remote probe answered without reporting its resolved
binary". It failed whenever Farhelm's ssh connection to that host was shared between commands, which is Farhelm's
normal setup.

Update checks the host with the same probe and could fail the same way. The probe now closes its input before it stops,
so the shared ssh connection releases the probe's error output, and anything already read is kept if that output still
stays open. The bound that discarded it came with the bounded probe cleanup of 2026-10-20 (#2415).
