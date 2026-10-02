---
kind: fixed
---

Marking a session read or unread can no longer crash the window when the session list goes away before the helm has
answered, for example when the browser asks for the sign-in token again at that moment. The choice is still sent to the
helm; only its success or error line is skipped, since there is no list left to show it on. Once you are signed in
again, the list shows whether it was saved.
