---
kind: added
---

Antigravity agents now support automatically suppressing the "trust this directory?" question. Enable it in Settings →
Agent trust.

The switch is off by default and applies to new Antigravity agent launches in any directory. Turning it on lets
repository hooks, settings and commands run without asking about workspace trust. The working directory replaces the
saved trust list for that launch. Custom permission flags with their own `--settings` take precedence. While the
question is on screen, the session shows Waiting and can offer a link to the switch.
