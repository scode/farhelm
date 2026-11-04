---
kind: added
---

You can now add a Tensorlake sandbox as a host from a saved recipe, with setup progress in Hosts. Creating it confirms
that it bills while running; deleting it also deletes its snapshots.

Tensorlake hosts can be suspended and resumed without restarting their sessions. The session launcher resumes a
suspended host before launching; a fresh GitHub checkout asks you to review its refreshed preview and launch again.

New Tensorlake hosts start YOLO sessions without asking. Reused images need systemd running as their init system and
must not contain an existing Farhelm supervisor installation.

Tensorlake host connections left behind when the helm goes silent now close after two minutes, so a sleeping helm does
not keep a sandbox awake indefinitely. Both ends need this support; ordinary SSH hosts keep their existing behavior.

A recipe can define tunnels that let anything in the sandbox reach a service on the helm's machine, such as an API
proxy that the sandbox's agents use, while the host is connected. A tunnel that cannot open is shown on the host's row
and retried. Tunnel connections go directly to Tensorlake's gateway and do not use a jump host from your ssh
configuration.

Settings → Tensorlake lists Farhelm sandboxes in your account that no host or snapshot build on this helm owns, such as
ones left behind when a helm lost its state, and can delete them with their snapshots. Sandboxes created by another
helm are marked, since they may still be in use there.
