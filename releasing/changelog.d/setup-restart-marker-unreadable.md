---
kind: fixed
---

`farhelm helm setup` now stops with an error when it cannot read the marker recording a service restart owed by an earlier failed run, instead of skipping that restart and reporting success while the service kept running its old configuration.
