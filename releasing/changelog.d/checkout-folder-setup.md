---
kind: added
---

Set or clear the folder used for managed checkouts on every host from Settings, or set it directly in the session launcher when no folder is configured. Refused values have a visible explanation; saving from the launcher refreshes its repository suggestions and preview. Farhelm creates a missing folder and its parents on the first checkout there; a remote host must run this release for that to work on it. Hosts given their own folder from the command line keep it; per-host folders and post-clone commands remain command-line settings.
