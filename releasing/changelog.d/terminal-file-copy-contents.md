---
kind: added
---

You can now Shift-Cmd-click a file path in the terminal (Shift-Ctrl-click on Linux) to copy the file's contents into
your clipboard, without saving anything to Downloads. Plain click still downloads the file, and Cmd-click (Ctrl-click
on Linux) still opens allowed types. Copying works for files up to 10 MB that are text or, on a Mac, PNG, JPEG, WebP,
GIF, BMP, TIFF or ICO images; anything else is refused with a reason. On Linux only text can be copied for now.

The plan's release note mentions only the Mac gesture and says nothing about the 10 MB cap, the list of formats, or
that Linux copies text only; those limits are what was built, as the maintainer decided when planning it.
