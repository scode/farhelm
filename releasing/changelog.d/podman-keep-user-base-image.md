---
kind: fixed
---

Deleting or rebuilding a container recipe no longer deletes your own base image along with Farhelm's images. When the
base was a local image you chose by its ID rather than by a name, removing Farhelm's last image built on it could
remove the base image too.
