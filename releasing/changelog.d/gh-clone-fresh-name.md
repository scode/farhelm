---
kind: fixed
---

Cloning a session, or using Replace with, into a fresh GitHub checkout no longer fails with "a directory with that
checkout name already exists". The name the clone copied from the original session is no longer used for the new
checkout: unless you type a name, the session gets the next free `repo-N`, the same as a new session with no name, and
the name field shows that name until you type one. This also applies to sessions you renamed and to cloning a session
that was not itself a checkout.
