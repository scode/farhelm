---
kind: removed
---

The add host dialog now asks only for the ssh destination. Its **remote farhelm (optional)** and **remote state dir
(optional)** fields are gone: Farhelm finds an existing installation where its own setup puts one, and records where
setup installed it. A host that runs Farhelm from somewhere else, or with its own state directory, is no longer found
from the dialog, which offers to set up a fresh copy instead.
