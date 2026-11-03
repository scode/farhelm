---
kind: none
---

Nothing user-facing yet: this lets the supervisor checkpoint each terminal running in a terminal host, so that a
restarted supervisor rebuilds the terminal from the latest checkpoint and the output after it, answering the terminal
queries a program asked while no supervisor was running, and never repeating an answer, a bell or a clipboard write.
A query in output the previous supervisor was sent but had not yet interpreted when it stopped goes unanswered. Output trimmed while the supervisor was away leaves the history marked as trimmed, and the
program is asked to redraw. Nothing in the product uses it until a later change turns new terminals on; that change
carries the real entry.
