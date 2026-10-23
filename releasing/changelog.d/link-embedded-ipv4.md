---
kind: fixed
---

A link an agent asks to open now counts as a local-network link, covered by **Open local-network links without
asking**, when it is an IPv6 address that stands for a local IPv4 address or an IPv6 address reserved for local use.
Before, such links counted as public, so a host that opens public links without asking opened them without a card.

The IPv6 forms are IPv4-compatible, NAT64 and 6to4 addresses whose IPv4 address is local, the local-use NAT64 prefix
and the deprecated site-local range.
