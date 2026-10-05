---
kind: changed
---

The installer and the helm now download Farhelm releases from get.farhelm.io instead of from GitHub. The installer
learns the latest release there, gets the release's checksums there, and fetches its archives through it; the helm gets
the files it pushes to a new host the same way, still checking them against the signed checksums. A release that is
not published on get.farhelm.io cannot be installed with the installer, and releases from before this change are not
published there.
