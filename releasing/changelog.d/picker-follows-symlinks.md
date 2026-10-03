---
kind: fixed
---

The folder picker in the new-session dialog now lists folders you reach through a symbolic link, such as a `~/src` that
points at another disk; before, it showed only real folders, and you had to type the path. A folder holding an entry
whose type cannot be read no longer fails to list as a whole; that entry is left out.
