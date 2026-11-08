---
kind: changed
---

New Tensorlake hosts start from a snapshot with Farhelm already installed, so they no longer install it during setup.
The first new host from a recipe waits while Farhelm prepares that snapshot, which briefly bills a preparation sandbox.
A later new host prepares it again only when it is more than 24 hours old, Farhelm was updated, or the recipe's
snapshot was rebuilt; nothing renews it in the background.

A prepared snapshot preserves any authentication already in the chosen custom base; its author remains responsible for
that content. Farhelm adds its own credentials and tunnels separately to each host.
