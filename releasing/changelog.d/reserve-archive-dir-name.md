---
kind: fixed
---

A fresh GitHub checkout can no longer be named `farhelm-archived-working-copies`, the folder deleted checkouts are moved into. A title such as "archived working copies" on a repository named `farhelm` produced exactly that name; later deletes then moved other checkouts inside it, and its own session could never be deleted or restarted. Such a title is now refused as taken, and you choose another.

If a checkout already has that name from an earlier version, deleting a session no longer moves other checkouts into it, and deleting its own session reports that the folder must be moved or renamed by hand instead of failing with "Invalid argument" on every attempt.
