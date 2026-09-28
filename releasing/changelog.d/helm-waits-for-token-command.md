---
kind: fixed
---

The helm (including the desktop app's built-in helm) no longer fails to start with "another process owns token control" when `farhelm helm token show` or `farhelm helm token rotate` happens to run at the same moment, as it easily can right after `farhelm helm setup`, which suggests running `token show` next. Startup now waits a few seconds for the token command to finish, and only reports another owner if the state directory stays claimed.
