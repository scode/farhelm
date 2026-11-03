---
title: Move sessions to new terminals
description: Why Farhelm's terminals no longer run in tmux, what that means for sessions you already have, and how to move them.
sidebar:
  order: 11
# A draft until the release that turns new terminals on, which describes the world after it: production builds leave
# drafts out. That release:
# - removes `draft: true`;
# - adds a link here from "Session details and actions" in work-in-a-session.mdx (the bar under the terminal now names
#   terminal kinds);
# - corrects the published pages that say every session runs in tmux: get-started/install.md, "If the installer asks
#   for tmux" ("Farhelm runs every session inside tmux"), and agents/agent-wrappers.md ("tmux starts the pane's login
#   shell");
# - checks this page against what shipped: the known gaps against TODO.md's near-term entries, the tmux paragraph
#   against what the installer and host setup do then, and the remark on remote hosts with an older supervisor against
#   how those hosts are handled.
draft: true
---

Farhelm has changed how it runs your sessions' terminals. Until now, every session's terminal, and every shell tab
beside it, ran inside tmux on its host. New terminals run without tmux: each one runs in a small Farhelm process of its
own on the host, which keeps it going while the host's [supervisor](/docs/how-it-works/the-pieces/) restarts or updates.
A host makes new terminals once its supervisor is a version that does; a remote host still on an older one keeps making
old terminals until you [update it](/docs/using/manage-hosts/#update-a-host).

Sessions you already have keep their old terminals and keep working as they always have. Nothing is moved for you,
because moving a session's agent means starting it again. You move each session when it suits you.

## Why terminals are leaving tmux

tmux was never something you were meant to deal with, but its quirks reached you anyway:

- A new tmux version could break Farhelm. One release changed a detail Farhelm relied on, and on every host running it,
  Restart, Replace, Stop and Delete failed for any session whose agent had exited.
- Every session on a host shared one tmux. Upgrading tmux, or a tmux crash, ended all of them at once, much like a
  reboot.
- Two programs kept their own copy of each terminal's screen and could disagree about how wide a character is, which
  left the cursor in the wrong place after some emoji and symbols.

A new terminal stands on its own: a problem with one does not take the others down, and it no longer depends on which
tmux version a host has.

## What an old session shows

When a session on a host that makes new terminals still has an old terminal, an amber note appears at the top of the
session:

- If the agent's terminal is old, it suggests restarting the session when convenient. For a session that cannot be
  restarted, it suggests cloning or replacing it instead.
- If the agent already runs in a new terminal but some shell tabs are old, it names those tabs and suggests closing them
  and opening new ones.

**What's changing** opens this page. The **×** hides the note until you next open the session.

The bar under the terminal also says which kind each of the session's terminals is: old, new, or both. Hover over it to
see each terminal listed, the agent first and then **Terminal 1**, **Terminal 2**, and so on, as the tabs are named
above the terminal.

## Move a session

On a host that makes new terminals, everything Farhelm starts gets one: new sessions, restarts, clones, replacements and
new shell tabs. So any of these moves the agent:

- **Restart** starts the agent again in a new terminal and continues its conversation. It is the usual way, where the
  session offers it; [Stop, restart, and resume](/docs/using/stop-restart-resume/) says when it does.
- **Replace** starts a new session in this one's place, in a new terminal, with a fresh conversation unless you pick a
  conversation to continue (with **Replace with…**). The old session, its shell tabs and its terminal history go.
- **Clone** starts a new session beside this one, in a new terminal, and leaves this one alone, so you can move over
  first and delete the old one later.

Restarting the agent does not move its shell tabs: an old tab stays old until you close it. When convenient, close each
old tab and click **+** to open a new one;
[Work in a session](/docs/using/work-in-a-session/#open-a-shell-beside-the-agent) covers tabs. A session can have old
and new terminals side by side meanwhile.

## What is the same, and what is not yet

Old terminals keep working exactly as before, so there is no hurry. You use both kinds the same way: type to the agent,
scroll back, select and copy, paste text, screenshots and files, and open links.

A few things do not work in a new terminal yet:

- Dropping a file onto it. Farhelm says to paste the file instead, which does the same.
- Typing with an input method, as for Chinese, Japanese or Korean. What you commit reaches the agent, but the text you
  are composing, and the input method's list of candidates, do not appear at the terminal's cursor while you type.
- The hint after a drag that copied nothing. In an old terminal, when a program that uses the mouse keeps a plain drag
  from selecting text, Farhelm shows where to hold Shift (Option on a Mac) to select anyway. A new terminal does not
  show that hint, though holding the key works the same.

One display difference has no decided fix yet: an emoji drawn wide by a variation selector, as in ❤️, takes one column
in a new terminal where tmux gave it two, so a program that counts two columns for it can leave the rest of that line
one column off.

## tmux is still needed

Farhelm still needs tmux on your Mac, as before, even after you move every session: keep it installed.
[Install Farhelm](/docs/get-started/install/#if-the-installer-asks-for-tmux) says which version. On a remote host,
nothing changes either: Farhelm keeps using the tmux it found there, or the copy it brought along when the host had no
new enough tmux ([Add a remote host](/docs/get-started/add-a-remote-host/) covers that). A later release will say when
Farhelm stops needing tmux.
