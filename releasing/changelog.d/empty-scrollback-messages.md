---
kind: fixed
---
A broken or malicious host could make a link in a terminal's scrollback open a different address from the one shown,
without showing where it went. Links in scrollback now always open the address they show.

The host had to send empty scrollback updates while earlier scrollback was still being loaded; ordinary Farhelm hosts
never send them. This affected new terminals only (those running in a terminal host); the bug is present in v0.37.1.
