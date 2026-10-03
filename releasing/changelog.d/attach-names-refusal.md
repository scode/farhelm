---
kind: fixed
---

Setting up or updating a host whose new supervisor Farhelm refuses now fails at once with the reason and what to do
about it, instead of keeping the host busy for 30 seconds and then failing as "timed out". The reasons are the ones the
hosts panel already shows: the supervisor speaks a different protocol version (the message names both versions), it
reports a different identity than the one on record, it reports no identity at all, or its identity belongs to another
host entry. Updating a host that is out of date still works: the old version mismatch it had before the update does not
stop the run.
