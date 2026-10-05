---
title: Approve what agents do
description: How Farhelm asks you before an agent starts, stops, or changes sessions, and how to stop it asking for a host.
sidebar:
  order: 9
---

An agent running in a Farhelm session can do more than work in its own terminal. With the `farhelm` command, which it
uses when you ask it to (with `$farhelm` in your message) but can also run on its own, it can look at your hosts and
sessions, start new sessions on any host, copy a session to the same host or another, rename, stop, or restart sessions,
and change your [launch templates](/docs/using/launch-templates/). Farhelm asks you before any of those changes happens,
whoever typed the command; a command you type yourself in a session's terminal is asked about the same way. Looking
never asks.

An agent can only do this from a session that is open in the Farhelm window. If it is not, the agent is told to ask you
to open it.

## The card

When an agent asks to change something, a card appears in the bottom-right corner of the Farhelm window. It says what
the agent wants to do and shows the details that matter:

- which session asked, and from which host;
- the session it would act on, for a rename, stop, or restart;
- for a new session, the host, the directory, the agent and its choices or the full command it would run, and whether it
  would run in [YOLO mode](/docs/using/start-a-session/#confirm-a-yolo-launch) (no approval prompts);
- for a template change, the whole template as it would be saved, including any command line.

Most of what a card shows was written by the agent: session titles, directories, commands. Read those as the agent's
words, not Farhelm's; each one sits under its own label.

The card has three buttons:

- **allow** lets this one request go ahead.
- **always allow from** the host lets this request go ahead and turns on that host's **run farhelm commands from this
  host without asking** setting, so agents there stop asking (see below).
- **deny** refuses it, and the agent is told you declined.

The rest of Farhelm stays usable while cards wait, and several can wait at once. Cards stay clickable even over an open
dialog. When the cards move, because one was answered or a new one arrived, their buttons pause for a moment, so a
double-click cannot answer a card you have not read.

## When nobody answers

The agent waits up to nine minutes for your answer. If nobody answers by then, the card disappears and the agent is told
so; nothing was changed, and the agent can ask again. If no Farhelm window is open when the agent asks, it is told
straight away to ask you to open Farhelm. On your Mac, quitting Farhelm or closing its window counts as no window.

A card stays up even if the agent gives up waiting for it before the nine minutes are over, and allowing it then still
does what it says. Deny cards you no longer want.

If the session that asked is deleted while its card waits, the request is refused.

## Stop asking for a host

Each host has a **run farhelm commands from this host without asking** setting, in its settings next to the YOLO one
([Manage hosts](/docs/using/manage-hosts/#host-settings)). Every host starts with it off. Turning it on means agents in
that host's sessions start, stop, restart, rename, and copy sessions on any of your hosts, and change templates, without
asking you. Turn it on only for a host whose agents you would let do that unsupervised.

If you **adopt** a host that was reinstalled or replaced at the same address, the setting goes back to off, like the
YOLO setting: you have not decided about the new installation yet.

## YOLO sessions and custom commands

On a host that asks you to [confirm YOLO launches](/docs/using/start-a-session/#confirm-a-yolo-launch), an agent cannot
start a YOLO session at all, with or without your approval. It cannot start a session that runs a
[custom command](/docs/agents/custom-commands/) there either, because Farhelm never reads a command to check whether it
runs without approval prompts, and an agent's word on that is not enough. The same goes for copying a session that an
older version of Farhelm started, whose command line may not be the one Farhelm would put together today. To let agents
do any of these on a host, turn on its **start YOLO sessions here without asking** setting.

[Restarting](/docs/using/stop-restart-resume/) a session that already exists is not affected: a restart resumes the
session with the settings it has saved, and is asked about like any other request.

## Next

[Manage hosts](/docs/using/manage-hosts/) covers the rest of a host's settings, and
[Launch templates](/docs/using/launch-templates/) what a template holds.
