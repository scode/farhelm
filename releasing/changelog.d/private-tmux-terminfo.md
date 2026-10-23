---
kind: fixed
---

On Linux hosts where Farhelm installs its own tmux rather than using the distribution's, that tmux now looks for
terminal definitions in the usual system locations. Before, it looked only in a temporary folder from its own build,
which no longer exists and which another account on the host could create, so attaching to a Farhelm session's tmux by
hand from a terminal failed to find the terminal's definition or could read a planted one.

Farhelm itself never runs this tmux in a terminal, so sessions in the app were not affected.
