---
kind: changed
---

The desktop app no longer passes the helm's web token into its window. When the window needs a new login, the app
creates it itself and gives the window only that login, so content the window shows cannot capture the web token.
