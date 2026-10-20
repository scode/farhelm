---
kind: fixed
---

Pins and read state stay with a session's host entry, so removing a host and adding it again starts them fresh. The
upgrade also resets pins and read state that Farhelm cannot attribute to one host.

Read state includes the seen/unseen stamp and notification read/cleared marks.
