---
kind: fixed
---

On Linux with systemd, Farhelm stops Podman containers started by an agent or terminal tab when it cleans up that
agent or tab. Podman removes containers started with `--rm` and keeps the others in the exited state.

The app warns when Podman cannot finish the cleanup so you can check the affected containers on their host.
