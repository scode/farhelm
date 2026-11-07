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

Claude Code and Codex can use named subscription logins from Tensorlake recipes. Claude may show its own first-run
setup once on a new host. Farhelm preserves Codex's refreshed login file; switching away leaves it on the host, so
signing out there is still up to you.
