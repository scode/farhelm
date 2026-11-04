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

One display difference has no decided fix yet: an emoji drawn wide by a variation selector, as in ❤️, takes one column
in a new terminal where tmux gave it two, so a program that counts two columns for it can leave the rest of that line
one column off.

## tmux is still needed

Farhelm still needs tmux on your Mac, as before, even after you move every session: keep it installed.
[Install Farhelm](/docs/get-started/install/#if-the-installer-asks-for-tmux) says which version. On a remote host,
nothing changes either: Farhelm keeps using the tmux it found there, or the copy it brought along when the host had no
new enough tmux ([Add a remote host](/docs/get-started/add-a-remote-host/) covers that). A later release will say when
Farhelm stops needing tmux.
