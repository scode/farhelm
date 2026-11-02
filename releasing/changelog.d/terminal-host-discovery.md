---
kind: none
---

Nothing user-facing yet: this adds the supervisor's discovery of terminal hosts (finding the hosts in a state directory
again by what their tags say, keeping hosts it cannot read untouched as unavailable, and safely removing the sockets
that killed hosts leave). Nothing in the product uses it until a later change turns new terminals on; that change
carries the real entry.
