---
kind: changed
---

This release changes how the helm and its hosts talk to each other, so a helm works only with hosts that speak the same
version of that connection: a helm from an earlier release shows a host updated to this release as **too new** and
cannot use it until the helm is updated, and this release's helm shows a host still on an earlier release as **needs
update** until that host is updated.

For the curator: this is the helm-supervisor protocol bump (67 to 68) for the coming terminals that do not run in tmux
(grid streams). Releases that share a protocol version keep working together, so later releases on 68 need no such
caveat. Nothing else changes for users yet; the change that turns those terminals on amends this fragment with what
users see.
