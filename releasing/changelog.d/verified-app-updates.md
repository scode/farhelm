---
kind: changed
---

The Mac app now checks get.farhelm.io for updates and verifies each release before installing it: the release's
checksums must be signed with one of the keys built into the app, and the installer must match them, or nothing is run.
If a release cannot be verified, the version in the sidebar shows a warning that this Farhelm can no longer
verify its updates, with the command to reinstall it.
