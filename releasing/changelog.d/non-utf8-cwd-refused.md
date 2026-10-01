---
kind: fixed
---

A session whose folder's real location has a name that is not valid UTF-8 (for example a symlink pointing into such a
folder) is now refused when you create it, with a message saying why. Before, creating it worked, but every later
restart reported success while starting the agent in your home folder instead. Restarting such an existing session is
now refused the same way.
