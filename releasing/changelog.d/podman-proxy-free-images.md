---
kind: fixed
---

Container recipe images no longer keep the proxy settings, proxy passwords included, of the container host they were built on. New containers no longer start with those settings. A recipe image built by an earlier release still holds them in the container host's Podman storage until the recipe is rebuilt.

Context for curation: earlier builds left the build-time proxy file inside both the recipe's software image and its prepared image, in the host account's Podman storage. After upgrading, the next launch prepares a new image on the recorded software image, and that image hides the file, but it shares the older software image's layers, which hold it until a Rebuild; the maintainer chose not to force those rebuilds.
