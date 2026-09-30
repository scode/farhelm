---
kind: fixed
---

When a fresh GitHub checkout's root is on a filesystem that cannot do the no-overwrite move Farhelm archives with (for
example some network or FUSE filesystems), or the archive folder cannot be written, deleting the checkout's last session
now says plainly that the checkout was not moved and stays where it is. An archive on such a filesystem that was
interrupted by a crash no longer keeps its session from being restarted.
