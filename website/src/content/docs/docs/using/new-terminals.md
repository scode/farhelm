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

An upcoming Farhelm release will stop supporting old terminals and will forcibly terminate anything still running in
one. Restart, replace, stop or delete each session that still runs in an old terminal, and close and reopen any of its
shell tabs that are still old terminals, as soon as possible (restarting a session restarts only its agent, not its
tabs). Each such session shows a loud note at the top and a notification in the session list.

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

When something in a session on a host that makes new terminals still runs in an old terminal, an amber note appears at
the top of the session. It asks you to act as soon as possible, because an upcoming Farhelm release will forcibly
terminate whatever still runs in an old terminal:

- If the agent is running in an old terminal, it asks you to restart (the conversation resumes), replace, stop or delete
  the session. For a session that cannot be restarted, it asks you to replace, stop or delete it instead, and says why
  it cannot be restarted: either Farhelm does not know the agent's conversation yet (most agents report it after your
  first prompt), or the session has no way to resume one.
- Otherwise, if some shell tabs are old, it names those tabs and asks you to close them and open new ones. Restarting
  the session would not help with these, since it restarts only the agent, not the tabs. This is the note you see right
  after restarting a session that had old tabs.

A session whose agent has stopped, and that has no old tabs, shows no note even if the agent's terminal is old: nothing
is running in it any more, and restarting the session later gives the agent a new terminal.

So that you hear about sessions you never open, each such session also gets a notification: a red bell on its row in the
session list ([Read the session list](/docs/using/session-list/#when-a-row-shows-a-bell) says how the bell works). It
says what the note says, in one message for the agent and the tabs, and ends with a link to this page. A session gets
only one such notification, even when restarting it leaves old tabs open. Once nothing in the session runs in an old
terminal any more, the notification is greyed and marked **Resolved**.

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
