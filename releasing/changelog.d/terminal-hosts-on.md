---
kind: changed
---

New sessions and terminal tabs no longer run inside tmux, and restarting a session from an earlier version moves it out
of tmux too; [Move sessions to new terminals](https://farhelm.io/docs/using/new-terminals/) explains what changes and
how to move your existing sessions.

Further caveats, for the curator to keep with the entry: sessions that were already running keep working in tmux until
they are restarted, cloned or replaced, and show a note at the top saying so and that a future release will stop
supporting old terminals; the bar under the terminal says which of a session's terminals are old and which are new.
tmux is still required. In the new terminals, typing with an input method works but the text being composed is not
shown at the cursor; dropping a file onto the terminal is refused with a message saying to paste it instead; a plain
drag over a program that uses the mouse copies nothing and no longer shows the notice saying how to select instead; and
an emoji followed by the emoji variation selector (as in "❤️") takes one column where tmux gave it two, so a program
that counts two finds the rest of that row one column off. Helms and hosts must be on releases that speak the same
version of their connection: a helm from an earlier release shows a host updated to this release as **too new** until
the helm is updated, and this release's helm shows a host still on an earlier release as **needs update** until that
host is updated.

For the curator: this is the switch-on of the terminal-host stack. Each new terminal runs in a small Farhelm process of
its own (a terminal host) that keeps it running with its screen and history across supervisor restarts and updates, and
Farhelm draws it itself; the supervisor's hidden `--new-terminals` option now defaults to hosts, and no shipped launch
passes it. The protocol caveat above is also `grid-protocol.md`'s entry, which covers both directions; keep it once. The
note at the top of old sessions and the bar's terminal kinds came with `terminal-migration-band.md`, and moving a
session by restarting it with `old-session-restarts.md`; the stack's other kind:none fragments point here too. The four
gaps are `TODO.md` near-term entries; re-check the list against `TODO.md` at release time and drop any that were
fixed.

For the curator: in v0.34.0-rc.1 and rc.2, restarting an old session on a Mac failed with "tmux described the pane in an
unexpected shape" and left it in tmux, because the supervisor the app starts has no UTF-8 locale and tmux then mangled
the output Farhelm reads (a Linux host whose supervisor has no UTF-8 locale had the same problem); fixed before the
stable release, so the entry needs nothing for it. The same mangling turned non-ASCII characters in a tmux pane's title
into `_`, and Codex's status reading uses a spinner in the title, so old Codex sessions on a Mac may have shown as idle
while Codex streamed a reply, in earlier releases too. That last part is read from the code, not observed; verify
before mentioning it.
