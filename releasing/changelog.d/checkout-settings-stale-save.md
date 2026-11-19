---
kind: fixed
---

If you change a checkout folder or post-clone command from the command line while its editor is open, the app now keeps the command-line change and reloads the field when you try to save.

Caveat for curation: a refused save now discards the edit and shows the stored value again, with the reason. That
includes a conflict, where a long post-clone command typed in the field is lost and must be typed again, and a refusal
for an invalid value such as a relative folder, which before kept the typed text for correction. Both follow SPEC.md's
rule for compare-on-save settings.
