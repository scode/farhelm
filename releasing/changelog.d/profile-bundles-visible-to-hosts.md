---
kind: changed
---

Treat saved profiles as visible to every attached host. Any host connected to your helm can read the full command line of any saved profile, the same way it could already have any profile launched on itself, so a key written into a profile's command line is not hidden from a compromised remote machine. The helm now logs each time a host reads a profile this way. This stays until spawning sessions on other hosts is limited to trusted environments.
