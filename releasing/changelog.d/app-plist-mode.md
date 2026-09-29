---
kind: fixed
---

On macOS, the installer now always makes the Farhelm app's `Info.plist` writable only by you. Before, if you installed with a permissive umask such as `002`, other accounts on the same Mac could edit it and make their own code run inside Farhelm the next time you opened it. Re-running the installer fixes an existing install.
