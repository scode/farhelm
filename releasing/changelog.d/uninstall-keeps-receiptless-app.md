---
kind: fixed
---

On macOS, `farhelm uninstall` refused to run at all when `~/Applications/Farhelm.app` had no Farhelm installer record,
which is the case for an app built by installer releases from before those records existed, or one you built yourself.
The suggested reinstall did not help when the app is turned off with `FARHELM_NO_APP_BUNDLE=1`, and would have replaced
an app you built yourself. Uninstall now leaves such an app in place, removes the rest of the installation, and says that
it kept the app.
