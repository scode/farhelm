---
kind: fixed
---

`farhelm agent restart` without `--stop-if-running` no longer kills a session's agent when the session's launch was never confirmed (it shows as Unknown). A create that failed ambiguously could leave such a session with its agent possibly still running, and a restart without consent used to stop that agent and its processes anyway. It is now refused until you confirm stopping it. Restarting an Interrupted or Exited session after a reboot still asks nothing, and the web and desktop UI already asked before restarting an Unknown session.
