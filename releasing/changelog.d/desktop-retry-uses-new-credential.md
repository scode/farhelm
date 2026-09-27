---
kind: fixed
---

In the desktop app, the first action after rotating the helm's web token (or after the app's saved sign-in was dropped because too many devices had signed in) no longer fails with "authentication is required". The app already fetched a new sign-in for such a request and retried it, but the retry still sent the old one first, so it was refused again.
