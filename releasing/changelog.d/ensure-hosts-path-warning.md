---
kind: fixed
---

The helm's startup warning about a host in the ensure-hosts file with a remote path that is not absolute no longer tells
you to repair the host with a set up action that remote hosts do not offer. It now says to correct the file, then remove
the host and restart the helm so it is registered again from the file.
