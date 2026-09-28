---
kind: fixed
---

On a large fleet, the web and desktop UI no longer replaces the session you just opened with the one it remembered from last time. When the remembered session was not in the first page of the session list, the UI looked it up in the background, and if you clicked another session before that lookup finished (it can take up to a minute), it switched to the remembered one anyway. A lookup that merely fails or times out also no longer makes the UI forget the remembered session for the rest of the visit.
