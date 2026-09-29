---
kind: fixed
---

Two runs of the install script started at the same time after an interrupted install could both try to undo the
interrupted one, and the second could delete the `farhelm` binary the first had just restored. Recovery now happens in
one run only; a run that starts while another is recovering stops and asks you to retry.
