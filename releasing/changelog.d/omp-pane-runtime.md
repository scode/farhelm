---
kind: fixed
---

In a rare OMP setup (a session started with an extremely long command line, running another interactive OMP inside it),
Farhelm could record the inner OMP's conversation as the session's own, so Resume reopened the wrong conversation. That
shape is now refused, and the session keeps its previous Resume offer.
