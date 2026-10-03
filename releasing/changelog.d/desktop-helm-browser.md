---
kind: removed
---
The desktop app's internal helm no longer serves a browser page or accepts browser token sign-ins. To use Farhelm from a browser, quit the app and run a standalone `farhelm helm run` on the same state directory. The app now chooses its own private port each launch; the `FARHELM_DESKTOP_PORT` and `FARHELM_DESKTOP_UI_DIST` overrides are removed.
