---
kind: fixed
---

A session created or restarted in a folder whose name contains `#`, such as `C#Samples` or `issue#12`, now starts in that folder. Before, the agent silently started in your home folder while Farhelm showed the folder you picked, and a folder name containing `#(…)` ran the text inside as a command when the session was created and again at every restart.
