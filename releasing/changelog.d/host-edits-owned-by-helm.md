---
kind: fixed
---

Adding, retargeting, removing or adopting a host in the hosts panel now always finishes on the helm even if the browser
disconnects or the page reloads mid-request. Before, the change could be saved without the helm acting on it: a new host
that never connected and could not be added again, a host still connected to its old address, a removed host still
connected, or a host stuck asking to adopt an identity it had already adopted.
