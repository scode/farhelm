---
kind: fixed
---
Updating a host added while it ran an incompatible Farhelm no longer shows a false "host changed" warning when it reconnects. Real ssh destination or identity changes now show both values and advice, clearing once the host connects at the helm's version.

Context for curation: the warning spells out hidden or unusual characters in the two install identities it names, so two
different identities can never look the same (#2571, a follow-up to this unreleased change).
