---
kind: fixed
---

Reloading the page just as a host update started could leave the host showing a stuck "running" update and refusing
every later setup or update until the helm restarted. The update now starts and runs whether or not the page is still
waiting for the answer.
