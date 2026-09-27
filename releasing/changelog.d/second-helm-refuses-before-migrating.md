---
kind: fixed
---

Starting a second helm against a state directory another helm is already serving, for example the desktop app next to the helm service or a helm run with a different `--port`, now refuses immediately. Before, it first upgraded the shared database, applied `--ensure-hosts`, and connected to every host, and only then refused. A newer helm started beside an older running one could upgrade the database so that the older helm failed and could not start again.
