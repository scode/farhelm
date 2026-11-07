---
title: Move sessions to new terminals
description: Why Farhelm's terminals no longer run in tmux, what that means for sessions you already have, and how to move them.
sidebar:
  order: 11
---

Farhelm has changed how it runs your sessions' terminals. Until now, every session's terminal, and every shell tab
beside it, ran inside a Farhelm managed tmux on its host. xterm.js was also used in the UI to render the terminal.

As of 0.34.0, all newly created terminals (new agent sessions, new shell tabs) run without tmux, and no xterm.js is
involved either. Instead, each terminal is owned by a small Farhelm processes which does the bare minimum to maintain
the terminal - including for short periods of time while other Farhelm processes are restarting, such as during
upgrades.

Sessions you already have running when you upgrade to 0.34.0 keep their old terminals and keep working as they always
have for now - inside of tmux, and with xterm.js on the client.

However, a future Farhelm release will stop supporting old terminals entirely. Starting with 0.34.0 you will be strongly
nudged (with a loud bar at the top of terminals, that you can dismiss) to restart/replace terminals at your earliest
convenience. This is intended to allow you to pick a good time to "migrate" your terminals over, while allowing Farhelm
to drop tmux/xterm.js support within a tractable time frame.

## Why terminals are leaving tmux

tmux was intended to be as much of an implementation detail as possible, but it leaks through:

- Farhelm exercises tmux in ways normal users don't. We already triggered tmux crash bugs in early development which
  lead to setting a minimum tmux version of farhelm, and supporting an embedded musl build of tmux. This meant
  supporting both the embedded musl build and the native build. We already had one case of a newer tmux breaking
  behavior in certain environments.
- Every session on a host shared one tmux. Upgrading tmux, or a tmux crash, ended all of them at once, much like a
  reboot. The new tmux replacement is meant to almost never need to be upgraded - but if and when it does, you'll be
  able to do so on a per-terminal basis rather than globally for an entire host.
- The old system involved terminal emulations occurring in two places (xterm.js and tmux). The new system has terminal
  emulation happening in precisely one place - the client/UI has a much more basic character grid protocol native to
  farhelm.

A new terminal stands on its own: a problem with one does not take the others down, and it no longer depends on which
tmux version a host has.

## What an old session shows

When a session on a host that makes new terminals still has an old terminal, an amber note appears at the top of the
session:

- It says that old terminals are fine for now, but that a future Farhelm release will stop supporting them.
- If the agent's terminal is old, it asks you to restart, replace or delete the session when convenient. For a session
  that cannot be restarted, it asks you to replace or delete it instead, and says why it cannot be restarted: either
  Farhelm does not know the agent's conversation yet (most agents report it after your first prompt), or the session has
  no way to resume one.
- If the agent already runs in a new terminal but some shell tabs are old, it names those tabs and suggests closing them
  and opening new ones.

**What's changing** opens this page. The **×** hides the note until you next open the session.

The bar under the terminal also says which kind each of the session's terminals is: old, new, or both. Hover over it to
see each terminal listed, the agent first and then **Terminal 1**, **Terminal 2**, and so on, as the tabs are named
above the terminal.

## What is the same, and what is not yet

Old terminals keep working exactly as before for now. You use both kinds the same way: type to the agent, scroll back,
select and copy, paste or drop text, screenshots and files, and open links.

Typing with an input method, as for Chinese, Japanese or Korean, does not work fully in a new terminal yet. What you
commit reaches the agent, but the text you are composing, and the input method's list of candidates, do not appear at
the terminal's cursor while you type.

Emoji presentation selectors, as in ❤️, now give eligible characters two columns in a new terminal, and text
presentation selectors can make a wide character narrow. This follows Ghostty's defaults; tmux also widens characters
that are not eligible and does not narrow them.

New terminals drop some invisible formatting characters from program output, including zero-width spaces, text-direction
marks, line and paragraph separators, word joiners, byte order marks and Hangul fillers. Those characters are also
absent from scrollback, saved terminal contents and text you copy. What you type, your programs and their files are
unchanged.

Two edge cases still differ from Ghostty: with wrapping disabled, a printed space in the final column is treated as
empty when choosing which character a selector affects; and a terminal only one column wide drops a wide character
before wrapping. The
[Ghostty reference for the first case](https://github.com/ghostty-org/ghostty/blob/c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed/src/terminal/Terminal.zig#L1939-L1967)
and
[the second case](https://github.com/ghostty-org/ghostty/blob/c7f68a7f48c6a5b357689eb8c2328bc2dab5d8ed/src/terminal/Terminal.zig#L2189-L2262)
show the behavior Farhelm will revisit.

A new terminal is not tmux and does not claim to be: programs in it see `TERM_PROGRAM=farhelm`, with Farhelm's version
in `TERM_PROGRAM_VERSION`, and no `TMUX` (inside a [containerized session](/docs/agents/containerized-sessions/)'s
container, the agent gets `TERM` and `COLORTERM` but no `TERM_PROGRAM`). An agent started, or a tab opened, before you
upgraded keeps the earlier environment, which claims tmux, until it is restarted or closed. If a startup file of your
login shell starts tmux whenever it is not already inside tmux, it now does so in a new terminal too, including while
the agent starts. One that replaces the shell with tmux (`exec tmux`) keeps the agent from starting at all, and the
session's terminal shows your tmux instead. To leave Farhelm's terminals alone, have the startup file skip tmux when
`TERM_PROGRAM` is `farhelm`.

## tmux is still needed

Farhelm still requires tmux for now, and checks its version, even after you move every session: keep it installed. On
your Mac, Farhelm does not start without a new enough tmux;
[Install Farhelm](/docs/get-started/install/#if-the-installer-asks-for-tmux) says which version. Setting up a remote
host still checks the host's tmux too, and brings Farhelm's own copy when the host has no new enough one
([Add a remote host](/docs/get-started/add-a-remote-host/) covers that).

Farhelm's tmux only runs for old terminals, though. Farhelm runs no tmux on a host whose sessions all have new
terminals, and on a host that still has old terminals, Farhelm's tmux stops by itself once the last of them is gone
(every old session restarted, replaced or deleted, and every old tab closed). An old session cannot do without it: if
tmux is missing or too old on its host, Farhelm cannot reach that session's terminal and shows the session as exited.
While that host's old tmux is still running, the agent may still be running in it too, so Farhelm refuses to restart or
delete the session until a supported tmux is installed again; the refusal says how.

A later release will say when Farhelm stops needing tmux.
