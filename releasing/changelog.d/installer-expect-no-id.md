---
kind: fixed
---

When the Mac app updates an older Farhelm installation without an ID, the installer can now verify that the
installation still matches what the app checked.

The CLI rejected this record when the update script passed the expected value as a separate argument. The CLI boundary
test verifies the fix. The triage record leaves open whether an ordinary update from a published release reaches it.
