---
kind: fixed
---

Session stop, restart, delete, and cleanup now recheck the process they are about to stop, so a recycled process ID cannot make Farhelm signal an unrelated process tree.
