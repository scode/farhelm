---
title: Approve what agents do
description: How Farhelm asks before an agent changes sessions or opens links, and how to stop it asking for a host.
sidebar:
  order: 9
---

An agent running in a Farhelm session can do more than work in its own terminal. With the `farhelm` command, which it
uses when you ask it to (with `$farhelm` in your message) but can also run on its own, it can look at your hosts and
sessions, start new sessions on any host, in an existing directory or a new
[managed checkout](/docs/using/start-a-session/#start-from-a-managed-checkout), copy a session to the same host or
another, rename, stop, or restart sessions, and change your [launch templates](/docs/using/launch-templates/). It can
also ask to open a web link in your browser ([Open links from a remote session](#open-links-from-a-remote-session)).
Farhelm asks you before any of those changes happens, whoever typed the command; a command you type yourself in a
session's terminal is asked about the same way. Looking never asks.

An agent can only do this from a session that is open in the Farhelm window. If it is not, the agent is told to ask you
to open it.

## The card

When an agent asks to change something, a compact card appears near the top of the main area, below an open session's
tabs. It leaves the sidebar and the terminal's prompt clear. It says what the agent wants to do and shows the details
that matter:

- which session asked, and from which host;
- the session it would act on, for a rename, stop, or restart, and the command a restart would run;
- for a copy of a session, the session being copied;
- for a new session, the host, the directory, the agent and its choices, the full command it would run and the command a
  later restart would run, and whether it would run in [YOLO mode](/docs/using/start-a-session/#confirm-a-yolo-launch)
  (no approval prompts). For a new [managed checkout](/docs/using/start-a-session/#start-from-a-managed-checkout), the
  card also names the repository it would clone, such as `GitHub: acme/web`, above the exact directory the clone would
  go into;
- for a template change, the whole template as it would be saved, including any command line;
- for a link, the site your browser would contact, above the full URL. Check the site: a URL can be written to look like
  a different site than the one it opens.

Most of what a card shows was written by the agent: session titles, directories, commands. Read those as the agent's
words, not Farhelm's; each one sits under its own label.

The card has three buttons:

- **allow** lets this one request go ahead.
- **always allow from** the host lets this request go ahead and turns on that host's **Run farhelm commands without
  asking** setting for a session or template change, so agents there stop asking (see below). A link card turns on only
  that link's permission instead. Other cards from that host that are already waiting stay up until you answer them.
- **deny** refuses it, and the agent is told you declined.

The oldest waiting request opens first. Other requests appear as headers below it; click one to read and answer that
request instead. The card shows how many requests are waiting, and the request you chose stays open until it is answered
or expires. Then the oldest remaining request opens. A long request scrolls as a whole; its command text is shown in
full.

The rest of Farhelm stays usable while requests wait, and several can wait at once, up to four from one host. While four
are waiting, that host's agents are turned away until you answer one. Cards stay clickable even over an open dialog.
When the waiting requests change or you switch to another request, the answer buttons pause for a moment, so a
double-click cannot answer a request you have not read.

## When nobody answers

The agent waits up to nine minutes for your answer. If nobody answers by then, the card disappears and the agent is told
so; nothing was changed, and the agent can ask again. If no Farhelm window is open when the agent asks, it is told
straight away to ask you to open Farhelm. On your Mac, quitting Farhelm or closing its window counts as no window.

A card stays up even if the agent gives up waiting for it before the nine minutes are over, and allowing it then still
does what it says. Deny cards you no longer want.

If the session that asked is deleted while its card waits, the request is refused.

## Stop asking for a host

Each host has a **Run farhelm commands without asking** setting, in its settings next to the YOLO one
([Manage hosts](/docs/using/manage-hosts/#host-settings)). Every host starts with it off. Turning it on means agents in
that host's sessions start, stop, restart, rename, and copy sessions on any of your hosts, and change templates, without
asking you. Turn it on only for a host whose agents you would let do that unsupervised.

If you **adopt** a host that was reinstalled or replaced at the same address, the setting goes back to off, like the
YOLO setting and both link settings: you have not decided about the new installation yet.

## Open links from a remote session

When an agent or a command in a remote session tries to open a web link through `BROWSER`, Farhelm asks to open it on
your machine. The card names the session and host and shows the site it would open and the full URL. Only HTTP and HTTPS
links are accepted. **allow** opens it in your default browser from the Mac app, or in a new tab from Farhelm's web UI.
**deny** opens nothing. Commands can also ask directly with `farhelm agent open-url -- https://example.com`, from any
session, including ones on your Mac.

The host settings have two separate permissions, both off initially. **Open links without asking** covers public
addresses. **Open local-network links without asking** covers addresses on your own machine or private network; those
cards explain why they ask separately. **always allow from** turns on only the permission that link needs. The commands
permission above covers neither. Farhelm decides which permission a link needs from the address as written, without
looking up where a name points, so a public-looking name can still lead to your private network.

With the matching permission on, the Mac app opens the link immediately. Farhelm's web UI still shows an **open** button
because your browser requires a click; **dismiss** refuses the request. Turning on the local-network permission lets
agents on that remote host make your browser load addresses on your machine or network without another approval.

A running agent gets forwarding at its next launch; new terminal tabs get it immediately. Sessions on your Mac open
links in your browser directly, as they always have. Your own `BROWSER` setting wins, even when empty. If Farhelm is
installed under a path with spaces or other special characters on the host, its sessions get no forwarding. A command
that waits for Farhelm learns whether you allowed or refused; a caller that starts it in the background can report
success before you answer. Codex's browser login does that. Claude Code 2.1.296 on Linux does not read `BROWSER` the way
most programs do, so its own opener does not forward; use the explicit Farhelm command instead.

A login that redirects to `localhost` on the remote host still cannot finish in your local browser. Opening the link
does not connect that remote login callback to your machine.

## YOLO sessions and custom commands

On a host that asks you to [confirm YOLO launches](/docs/using/start-a-session/#confirm-a-yolo-launch), an agent cannot
start a YOLO session at all, with or without your approval. It cannot start a session that runs a
[custom command](/docs/agents/custom-commands/) there either, because Farhelm never reads a command to check whether it
runs without approval prompts, and an agent's word on that is not enough. The same goes for copying a session that an
older version of Farhelm started, whose command line may not be the one Farhelm would put together today. To let agents
do any of these on a host, turn on its **Start YOLO sessions without asking** setting.

[Restarting](/docs/using/stop-restart-resume/) a session that already exists is not affected: a restart resumes the
session with the settings it has saved, and is asked about like any other request.

## Next

[Manage hosts](/docs/using/manage-hosts/) covers the rest of a host's settings, and
[Launch templates](/docs/using/launch-templates/) what a template holds.
