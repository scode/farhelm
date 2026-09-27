---
kind: fixed
---

`farhelm helm token rotate` now refuses when the state directory it resolved holds no helm database, and names the path it checked. Before, pointing it at the wrong directory (a mistyped `--state-dir`, or a shell whose `XDG_STATE_HOME` differs from the one your helm runs with) created a new, empty helm there, printed a new token, and reported success, while your real helm kept the old token and every signed-in browser. If you rotated a token that way to revoke it, run the rotation again against the right state directory.
