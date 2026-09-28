---
kind: fixed
---

A tmux server that stops responding can no longer freeze every terminal on a host. Resizing a terminal, which also happens whenever one is opened, waited for tmux with no time limit while blocking typing, opening, and closing of terminals for every session on that host. It now fails after a short time instead, leaving that one terminal at its old size.
