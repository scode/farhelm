---
kind: breaking
---

Settings → Credentials now keeps named tokens and agent logins, with a warning that they are not encrypted on disk.
Tensorlake recipes select named endpoint tokens; existing recipe tokens are removed, so re-enter your endpoint tokens
in Settings → Credentials and choose their names in the recipes.

Replacing an endpoint token sends it to hosts using it, and disconnected hosts receive it when they next connect.
Delivery failures keep the saved value and show a warning in Credentials and on the affected host.

Tensorlake recipes can also select a named GitHub token for private checkouts and new agent terminals. New snapshots
include `gh`; reused images need it installed, and older hosts need Farhelm's Update before new terminals receive tokens.
