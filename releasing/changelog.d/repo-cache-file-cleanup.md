---
kind: fixed
---

Farhelm can rebuild a stuck repository cache during checkout. If the rebuild fails too, Farhelm keeps the previous
cache for the next attempt.

Startup cleanup can also remove an expired cache left as a file and carry on cleaning the other caches.

Caveat for curation: after updating, the first managed checkout of each repository on a host downloads it again in
full, even when the old cache was healthy, before the old cache is removed; it takes longer than usual once.
