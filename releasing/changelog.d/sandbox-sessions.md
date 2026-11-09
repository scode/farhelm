---
kind: added
pr: 3265
---

Experimental [sandbox sessions](https://farhelm.io/docs/using/tensorlake-sandboxes/) can create a Tensorlake sandbox for
each session from a saved recipe. They bill while running, Suspend is explicit, and Delete permanently removes the
sandbox and its files.

Session sandboxes suspend after 30 minutes without a connection from Farhelm. Agents in them cannot ask Farhelm to act.
Claude's first-run setup may appear for each new sandbox when using a subscription login rather than an API endpoint.
