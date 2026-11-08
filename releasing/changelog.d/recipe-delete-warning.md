---
kind: fixed
---

Deleting a Tensorlake recipe now always warns that any running build is cancelled and the snapshots Farhelm built for it are deleted, including when a build finished after the editor opened. User-supplied base images and snapshots are kept.
