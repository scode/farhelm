---
kind: changed
---

When Create SSH key in Settings → Tensorlake takes longer than the helm's reply limit, the message now says to select
Create SSH key again in a few seconds to show the key, and Settings no longer checks for the key on its own over the
next half minute.

Settings still reads the account once right after such a message, so a key that is already there shows at once. If that
read finds no key, the line saying to try again appears.
