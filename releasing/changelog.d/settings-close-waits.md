---
kind: changed
---

Closing Settings while a change is still being saved now waits for the helm's answer, and if the helm refuses,
Settings stays open on that page with the reason.

This covers a switch, a form's Save or Test, a credential or custom agent being deleted, and the Tensorlake API key's
Test and Clear. If everything went through, Settings closes, or asks about any changes still unsaved. Recipe builds and
deletes, leftover cleanup and Create SSH key carry on by themselves and do not hold Settings open.

Before, closing could go on over a save still out, and a refusal that arrived afterwards was never shown. When you
closed Settings and chose Save while a test was running, Settings showed a note asking you to close again later. That
note is gone, since Settings now waits for the test.

A replaced credential that could not reach every host while Settings was closing now keeps Settings open on the
Credentials overview, where those hosts are listed.
