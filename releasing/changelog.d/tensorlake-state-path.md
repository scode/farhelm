---
kind: fixed
---

Tensorlake connections now refuse a helm state folder whose path contains `%`, `"` or `\`, with a message explaining
how to fix it. These paths could prevent older supported OpenSSH versions from retaining the gateway's trusted key or
connecting successfully.

The refusal applies whichever OpenSSH version is installed, so a Tensorlake setup under such a path that connected
before, with a newer OpenSSH, stops connecting until the helm's state folder moves to a path without those
characters.
