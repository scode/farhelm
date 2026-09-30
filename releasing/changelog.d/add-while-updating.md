---
kind: fixed
---

Confirming "add host" for a host that is already being updated is now refused before anything changes. Before, the
refusal came only after the host's connection settings had been rewritten, which could break the running update and
leave the host unable to connect, and the confirmation could not simply be repeated afterwards.
