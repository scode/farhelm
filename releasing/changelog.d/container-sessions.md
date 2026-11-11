---
kind: added
---

Experimental container sessions run a session and its checkout in a disposable container on one of your Linux hosts.
The session launcher and saved templates choose the recipe and container host, and Settings → Recipes manages the
shared catalog for containers and Tensorlake sandboxes.

Container hosts require rootless Podman 4.9.3 or newer, cgroup v2 and a working systemd user manager. The first launch
with a recipe on a host builds its image. The checkout lives inside the container, and Delete removes its files,
including work you have not pushed. Suspend freezes processes while keeping memory in use. SSH hosts usually allow
about ten simultaneous container connections, subject to that host's SSH session limit. Agents in container and sandbox
sessions cannot ask Farhelm to act yet.

Container sessions support Suspend, Resume, Restart, terminal tabs, file links and localhost tunnels. Settings → Recipes
shows image preparation for each container host and the Tensorlake account, and Settings → Containers shows each host's
Podman availability and containers left outside this helm's session list.
