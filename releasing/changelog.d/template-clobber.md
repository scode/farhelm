---
kind: fixed
---

Saving a new template, or renaming one, while the Templates panel's list was still loading or had failed to load could
silently replace an existing template with the same name. The panel now refuses such a save until the list has loaded,
says why, and offers **retry** when loading the list failed. Saving changes to the template you opened under its own
name works as before.
