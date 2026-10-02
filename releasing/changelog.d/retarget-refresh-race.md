---
kind: fixed
---

Changing a host's destination at the exact moment the helm finished refreshing that host's session list, or finished
connecting to it, could briefly put the connection to the old destination back into use, so an action sent in that
instant could reach the machine you had just stopped pointing at. The old connection now stays withdrawn.
