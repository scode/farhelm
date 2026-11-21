---
kind: fixed
---

Remote Mac setup checks for Farhelm.app again when you confirm, so installing the app while the question is open keeps
setup from starting a second supervisor.
If the SSH name now reaches a different login, setup asks you to prepare a new confirmation.

Setup and Update probes for disposable environments now end after two minutes without input even if the helm has stopped
responding and the connection stays open.
