---
kind: changed
---

Updates and supervisor restarts spend less CPU time saving terminal history. Terminals with the most output to replay
are saved first, while small amounts of recent output are replayed when the supervisor starts again.
