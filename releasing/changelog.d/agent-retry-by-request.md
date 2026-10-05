---
kind: breaking
---

Retrying `farhelm agent create`, or `farhelm spawn` with launch flags, with the same idempotency key, host, templates and flags now returns the session the first attempt started however long ago that was. Before, this held only for 30 days when a template the create names had been edited in between, and a later retry was refused as a reused key. Retrying `farhelm agent clone` the same way now also returns the first copy when the session it copied has changed since. `farhelm agent create` now needs `--host` whenever it is given an idempotency key, because the key is kept on the host the first attempt reached and a host taken from a template could change between attempts. Refusals about an idempotency key no longer quote the key back. A retry whose first attempt was cut short before its session started is refused, rather than started, if a template change means it would now start something different from what you approved.
