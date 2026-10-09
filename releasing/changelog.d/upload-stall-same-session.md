---
kind: fixed
---

When adding or updating a remote host that allows only one ssh session per connection, an upload that stops growing now fails after a minute instead of possibly hanging forever. The stall timer still starts only once the uploaded file exists on the host.
