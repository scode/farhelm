---
kind: fixed
---

When the desktop app fails to sign its window in, it now shows a Retry button next to the error. Before, the window
stayed on the error until you quit and reopened the app. This mostly matters after the browser sign-in token was rotated,
when the app has to sign in again while it is running.
