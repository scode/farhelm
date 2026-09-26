---
kind: fixed
---

Cloning a profile-backed session could select the profile you last launched with instead of the clone's own. This
happened when you chose "other / command" before the launch dialog had finished loading your profiles. The dialog now
selects nothing in that case and asks you to choose.
