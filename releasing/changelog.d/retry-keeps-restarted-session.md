---
kind: fixed
---

A session whose first launch was interrupted, and which was then restarted with a restart whose outcome Farhelm could
not tell, could later be launched again from its original create when that create was retried. The retry now leaves
the restarted session alone and reports it. The sequence was found by reading the code and has not been reproduced.
