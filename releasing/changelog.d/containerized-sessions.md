---
kind: breaking
---

Containerized sessions now start from an image already in the host's local Podman storage instead of an image archive
file, so Podman no longer unpacks the archive on every launch; Farhelm never pulls or manages images. The **Image file**
setting is replaced by **Image**, an image name or ID: settings, templates and sessions saved with an archive path show
an empty **Image** and must get one before they launch again (edit the template, or use **Replace with** for an existing
session).

For curation: v0.32.0 shipped containerized sessions with the archive path, so this changes a stable release's
settings; it was first drafted as a follow-up to a release candidate. Farhelm refuses an image that is missing from that
host's local Podman storage. Rebuilding an image under the same name affects the next new container, including Replace
and Replace with; an existing container keeps its original image. Container startup diagnostics appear in the session's
terminal.
