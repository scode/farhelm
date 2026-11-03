---
kind: none
---

Nothing user-facing yet: the supervisor can now stream a terminal that runs in a terminal host to a viewer that can show
grid terminals, with its scrollback, flow control, the stall detach and typed and pasted input (a paste is written
whole once all of it has arrived, so that neither the terminal's answers to the program nor other input land inside
it, and one cut short is dropped), and a resize to the size a terminal already has no longer reaches the program. No
session runs in a host, and no helm asks for grid streams, until later changes turn them on; those changes carry the
real entry.
