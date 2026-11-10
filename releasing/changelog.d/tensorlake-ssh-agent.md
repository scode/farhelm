---
kind: fixed
---

Tensorlake snapshot builds and sandbox connections no longer consult your SSH agent, preventing a personal key loaded
there from jumping ahead of Farhelm's registered key. A configured certificate (even one whose private key is only in your agent), or a
hardware-token key, can still take priority and cause a gateway that rejects the first unknown key to refuse the
connection; those settings must be left out of the SSH configuration that applies to the Tensorlake host.
