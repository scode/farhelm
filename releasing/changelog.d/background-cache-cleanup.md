---
kind: fixed
---

Starting a session in a new GitHub checkout no longer triggers Git cleanup in its terminal. Farhelm consolidates its
repository cache in the background afterwards, including when prepared checkouts are disabled; a later launch can still
wait for cleanup already holding that cache's lock.
