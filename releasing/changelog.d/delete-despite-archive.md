---
kind: fixed
---

Deleting a session no longer fails when its fresh GitHub checkout cannot be moved into the archive folder, for example
because the checkout root was removed, recreated or unmounted. The session is deleted, the checkout folder stays where
it is and is no longer managed by Farhelm, and the session list shows a notice naming the folder and why it was not
archived.
