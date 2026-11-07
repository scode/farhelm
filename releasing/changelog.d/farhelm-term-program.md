---
kind: changed
---

Agents and terminal tabs in new terminals are now told they run in Farhelm's own terminal (`TERM_PROGRAM=farhelm`, with
Farhelm's version as `TERM_PROGRAM_VERSION`) instead of being told they run in tmux. If a login shell's startup file
starts tmux whenever it is not already inside tmux, it now does so in new terminals too, including while the agent
starts; one that replaces the shell with tmux (`exec tmux`) keeps the agent from starting.

Agents started before the upgrade keep the old environment until they are restarted, and tabs opened before it keep it
until they are closed. Old terminals (sessions still in Farhelm's private tmux) are unchanged. Claude Code 2.1.319, for
one, no longer shows its "tmux detected" scrolling hint in a new terminal (seen in the screens re-captured in a terminal
host for status detection's tests).
