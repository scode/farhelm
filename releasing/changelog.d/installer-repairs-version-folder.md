---
kind: fixed
---

Rerunning the Mac installer now repairs a damaged copy of the version it installs. Before, a rerun kept that version's
`farhelm` program whenever the file was present, even when it was empty or cut short, and reported success over a
Farhelm that could not start.

The installer now keeps the existing copy only when it matches the verified download exactly, and otherwise replaces it.
The review finding named a power loss during an earlier install as the way the file ends up empty; that macOS's file
system can leave it so is unverified, so the entry words the cause generally.
