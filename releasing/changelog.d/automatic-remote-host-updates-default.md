---
kind: changed
---

Farhelm now updates remote hosts automatically while the app is open, including if you previously turned updates off.
After upgrading, you can turn off **Update remote hosts automatically** under **Hosts** in Settings to keep updates manual.

Caveat for curation: the first time the upgraded app opens, hosts that need an update start updating right away, so
turning the setting off after upgrading governs only later updates. Automatic updates also skip a host running a
development build, so the installed app never undoes a devbuild's update of a shared host by itself.
