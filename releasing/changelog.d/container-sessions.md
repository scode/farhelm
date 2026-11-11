---
kind: added
---

Experimental container sessions run a session and its checkout in a disposable container on one of your Linux hosts.
They use shared recipes and support Suspend, Resume, Restart, terminal tabs, file links and localhost tunnels.

Container hosts require rootless Podman 4.9.3 or newer, cgroup v2 and a working systemd user manager. The first launch
with a recipe on a host builds its image. The checkout lives inside the container, and Delete removes its files,
including work you have not pushed. Suspend freezes processes while keeping memory in use. SSH hosts usually allow
about ten simultaneous container connections, subject to that host's SSH session limit. Agents in container and sandbox
sessions cannot ask Farhelm to act yet. Earlier experimental containerized sessions can now only be deleted.
