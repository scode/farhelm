---
kind: fixed
---

When an Update or Uninstall you started shares an SSH sign-in question with an automatic update, your action now gets
the answer first; the automatic update waits until your run finishes, or for up to 30 seconds. Turning **Tailscale
host** on or off no longer answers busy because an automatic update is waiting to sign in: it dismisses that question
and keeps one your own action is waiting on.
