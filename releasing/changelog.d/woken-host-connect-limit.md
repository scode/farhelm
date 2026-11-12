---
kind: fixed
---

A launch that woke a suspended Tensorlake sandbox to check out a GitHub repository now gives up after 60 seconds if the
window never sees the woken sandbox connect, and shows the session launcher again with an error, instead of waiting
forever. Before, New, quick launches and host changes in that window stayed unavailable for as long as it waited, and the
hidden launcher could not be cancelled.
