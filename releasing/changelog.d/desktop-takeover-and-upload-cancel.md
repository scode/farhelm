---
kind: fixed
---

If an attachment upload is cancelled while the host is saving the file, Farhelm now says the file may already exist
instead of reporting that it was not saved.

Context for curation: whether a cancel can reach that point in practice was not verified (the finding marked its
premise, a half-closed connection to the host, as unconfirmed). The same change makes a window that another window
takes over show the takeover notice even while it is still sending; that race cannot happen in the shipped desktop
app, which allows only one window, so it is not worth an entry.
