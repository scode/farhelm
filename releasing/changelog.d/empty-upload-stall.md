---
kind: fixed
---

Attachment uploads now send empty body chunks through the normal stall and abort handling, so a no-progress stream cannot keep the helm busy indefinitely. This is a defensive relay fix; whether the production HTTP stack can deliver an immediately-ready empty stream remains unverified.
