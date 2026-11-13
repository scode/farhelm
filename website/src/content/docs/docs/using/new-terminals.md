---
title: New terminals
description: Why Farhelm's terminals no longer run in tmux, what happened to sessions that still ran in an old terminal when you updated, and how to get them back.
sidebar:
  order: 11
---

Farhelm has changed how it runs your sessions' terminals. Until 0.34.0, every session's terminal, and every shell tab
beside it, ran inside a Farhelm managed tmux on its host, and xterm.js drew it in the UI.

Since 0.34.0, every new terminal (a new agent session, a new shell tab) runs without tmux, and no xterm.js is involved
either. Instead, each terminal is owned by a small Farhelm process that does the bare minimum to keep the terminal
running, including for short periods while other Farhelm processes restart, such as during updates.

Farhelm has since stopped supporting old terminals, the ones that still ran in tmux. Earlier releases warned about them
with a note at the top of each such session and a notification in the session list.

## What happened to old terminals

The first time a host's Farhelm starts on a release without old terminals, it ends the tmux that ran them, and
everything that still ran in them:

- A session whose agent was still running in an old terminal now shows as interrupted, with a notification saying so.
  **Restart** starts the agent again in a new terminal and resumes its conversation; if Restart is unavailable for the
  session, **Replace** starts it anew. The new terminal starts empty: the old terminal's scrollback and its last screen
  are gone.
- A session whose agent had already stopped or exited keeps its status and gets no notification, but its terminal no
  longer shows the agent's last screen, which lived in the old terminal. Restart starts it in a new terminal as usual.
- Old shell tabs are closed. Open new ones where you need them.

Programs that old agents and tabs had left running in the background are stopped too, wherever Farhelm can tell which
session they belong to. Any it cannot tell are stopped when you delete their session.

The old notification asking you to restart such a session is now greyed and marked **Resolved**
([Read the session list](/docs/using/session-list/#when-a-row-shows-a-bell) says how notifications work).

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

## What is different in a new terminal

Typing with an input method, as for Chinese, Japanese or Korean, does not work fully in a new terminal yet. What you
commit reaches the agent, but the text you are composing, and the input method's list of candidates, do not appear at
the terminal's cursor while you type.

Emoji presentation selectors, as in ❤️, give eligible characters two columns in a new terminal, and text presentation
selectors can make a wide character narrow. This follows Ghostty's defaults; tmux also widened characters that are not
eligible and did not narrow them.

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

Programs in a new terminal see `TERM_PROGRAM=farhelm`, with Farhelm's version in `TERM_PROGRAM_VERSION`. `TMUX` is
unset. If a startup file of your login shell starts tmux whenever it is not already inside tmux, it does so in a new
terminal too, including while the agent starts. One that replaces the shell with tmux (`exec tmux`) keeps the agent from
starting at all, and the session's terminal shows your tmux instead. To leave Farhelm's terminals alone, have the
startup file skip tmux when `TERM_PROGRAM` is `farhelm`.

## tmux is still needed

Setting up a remote Linux host still checks the host's tmux, and brings Farhelm's own copy when the host has no new
enough one. Nothing else does: the Farhelm desktop app and a Linux machine running the helm no longer need tmux. Remote
Macs set up over SSH do not require or install tmux ([Add a remote host](/docs/get-started/add-a-remote-host/) covers
both kinds of host).

Farhelm itself no longer runs tmux for any session: it ends the tmux it ran old terminals in when it updates, and never
starts one again.

A later release will say when setting up a remote Linux host stops needing tmux.
