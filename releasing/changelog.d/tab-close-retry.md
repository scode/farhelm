---
kind: fixed
---

Closing a terminal tab whose final cleanup fails now keeps the tab for another try, and Farhelm retries it on its own.
Before, a tab whose terminal had ended but whose leftover processes could not be confirmed gone was forgotten at once,
so nothing tried again, and when ending its terminal failed too, the error said its processes had been reaped.

The automatic retries now also back off properly when a retry's own cleanup fails, or when the first automatic cleanup
of a tab whose shell exited fails, instead of trying again on every cleanup pass. The retry is held in memory only: if
the supervisor restarts before it succeeds, deleting the session cleans up what remains.
