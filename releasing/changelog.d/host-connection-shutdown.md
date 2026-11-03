---
kind: fixed
---
The helm no longer starts another connection attempt to a host while shutting down or removing that host.

The release gate exposed a cancellation scheduling window: stopping the task supervising a host's connection did not
cancel the connection task until supervision was polled again. This change requests cancellation when the host's
registry handle is removed, before retiring the connection can wake another attempt. No session or remote supervisor
is stopped by this change.
