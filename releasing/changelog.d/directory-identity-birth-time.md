---
kind: fixed
---

Deleting a session no longer archives a folder you put in place of its checkout. If you removed a checkout Farhelm had created and cloned or created something else at the same path, deleting the old session could move your new folder into `farhelm-archived-working-copies`, because the new folder often reused the old one's identity number on disk. Farhelm now also compares the folder's creation time. Checkouts created before this version are still compared the old way.
