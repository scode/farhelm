---
kind: fixed
---

On a Mac, the installer now refuses to update Farhelm.app in place when the app, or one of the folders inside it that
the update writes into, is a symbolic link or not a folder at all, and changes nothing. Before, an update followed such a link and could write the new
program or icon into whatever folder it pointed to. Replace the link with a real folder, or move the app aside, and run
the installer again.
