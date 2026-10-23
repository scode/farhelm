---
kind: changed
---

When the browser asks you to sign in again (for example after `farhelm helm token rotate`), entering the new token now
reloads the page. Anything the page had not saved before the sign-in expired, such as an open dialog or a draft, is
gone, and the page starts over from what the helm reports.
