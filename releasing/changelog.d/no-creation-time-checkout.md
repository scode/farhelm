---
kind: fixed
---

On a filesystem that records no creation time for folders, deleting a session no longer moves a checkout folder
into the archive, and the checkout trash no longer deletes it: Farhelm cannot tell that folder from one you removed and
cloned again at the same path, so it now leaves the folder where it is and says so. Common Linux and macOS filesystems
record creation times and are unaffected.
