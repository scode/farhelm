---
kind: fixed
---

Farhelm offers one container host for each machine and account. If a container host's SSH destination starts reaching
a different machine or account, Delete keeps a newly created container's entry and explains how to retry.

Previously created containers keep their existing cleanup behavior.

Caveats for curation: creating containers and building their images now also need `sha256sum` and `tr` on the container
host, since Farhelm checks the host's machine and account in the same shell that runs each Podman command. If two
container hosts already reach the same machine and account, only one of them keeps accepting new containers; the
launcher and Settings show why for the other.
