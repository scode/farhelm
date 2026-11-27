---
kind: added
---

`farhelm agent hosts` now shows whether each host is a Tailscale host, in a new last column, `TAILSCALE`, reading `yes`
or `no`. Before, only `farhelm agent hosts --json` said so.

Context for curation: this follows #5126, which shipped in 0.46.0 and added the `tailscale` field to the JSON form only.
The JSON output is unchanged. A script that reads the plain table by column position still finds `ID`, `NAME`, `KIND`
and `STATE` where they were; one that took everything after `KIND` as the state now also gets the new column.
