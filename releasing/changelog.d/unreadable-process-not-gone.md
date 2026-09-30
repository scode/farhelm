---
kind: fixed
---

In rare cases Stop, Delete, Restart or closing a tab reported that nothing was left running while the terminal's own
processes were still alive, because the terminal's main process could not be read at that moment. These operations
now report a failure in that case and can be retried.
