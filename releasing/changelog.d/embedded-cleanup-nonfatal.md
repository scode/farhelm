---
kind: fixed
---

The helm no longer refuses to start when it cannot remove the `embedded-payloads` folder an older version left in its
state directory (for example because a file in it is owned by root). It logs a warning and leaves the folder for you to
remove.
