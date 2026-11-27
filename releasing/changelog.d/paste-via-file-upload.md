---
kind: added
---

You can now press Cmd+Shift+V on a Mac, or Ctrl+Alt+V on Linux, to paste the text on your clipboard into a session's
terminal as a file: Farhelm uploads the text to the session's host and types the file's path instead of pasting the text
itself, so a long block of text never reaches an agent as one giant paste. On Linux, Ctrl+Alt+V no longer reaches the
program in the terminal, which some editors use (Emacs scrolls the other window with it).

On a Mac the Edit menu has it too, as Paste via File Upload, and the Keyboard shortcuts dialog lists it in its Terminal
group. Two things depart from the plan's release note, which mentions only the Mac chord and calls the file temporary.
Linux uses Ctrl+Alt+V because Ctrl+Shift+V is already ordinary paste there. And the file is not temporary in the usual
sense: the text lands in the session's attachments as `pasted-<n>.txt`, like a pasted screenshot, and stays there,
through Stop, until the session is deleted. Screenshots and files on the clipboard upload exactly as they do with an
ordinary paste.
