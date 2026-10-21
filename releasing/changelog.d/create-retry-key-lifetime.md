---
kind: changed
---

Retrying a create with the same key after deleting its session now creates a new session; agent requests still need
approval. Refused creates keep their original answer for 24 hours, then the key can be used for a fresh attempt.

On first start, Farhelm removes old retry records for sessions already deleted and gives existing sessions' keys the
same lifetime. This cleanup is one-way: returning to an older build does not restore those deleted-session answers.
