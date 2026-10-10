---
kind: fixed
---

OMP sessions launched through a custom Bun or npm command now withhold Resume when the terminal's top Bun or Node process has unreadable arguments, so a second OMP running inside the session cannot have its conversation offered as the session's own. Sessions launched with the installed `omp` command behave as before.
