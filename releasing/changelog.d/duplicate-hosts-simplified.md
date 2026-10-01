---
kind: changed
---

A host entry that reaches the same machine as another entry now says which entry already holds it, by name, and asks you
to remove that entry or change this one's destination, then press Retry. This also covers an entry that used to show an
"adopt" choice that could never succeed, such as one you pointed at another entry's machine, or the old entry of a
reinstalled host you added again. Such an entry no longer rechecks on its own every 45 seconds; it waits for Retry, an
edit of the entry, or a restart.
