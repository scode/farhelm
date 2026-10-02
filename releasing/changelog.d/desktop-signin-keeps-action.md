---
kind: fixed
---

In the desktop app, the first Delete, Stop, restart, rename or new session after the browser sign-in token was rotated
now finishes and shows its result. Before, the app signed its window in again by reloading it, and that reload threw away
the action that ran into the new token, so it may or may not have happened and nothing on screen said which. The window
now signs in again in the background and stays as it was.
