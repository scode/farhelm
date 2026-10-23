---
kind: fixed
---

In Settings, the checkout folder and post-clone command can now only be saved after you edit them, and a value you did
not edit is never written back. Before, clicking **save** without editing wrote the value Settings showed when it
opened, which could undo a change made to either setting from the command line while Settings was open, and could strip
a trailing space from the checkout folder's name. An edited checkout folder still has surrounding whitespace removed.
