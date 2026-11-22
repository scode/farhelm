---
title: Antigravity
description: Launch Antigravity CLI with Farhelm and set up Restart.
sidebar:
  order: 10
---

Choose **Antigravity** on the session launcher's **Agent** tab to start the Antigravity CLI (`agy`). Install it and sign
in on each [host](/docs/using/manage-hosts/) where you run it. Use the launcher to pick a Gemini model and effort, or
leave them to Antigravity's settings. **Guarded** requests auto mode. Antigravity uses its default mode if auto mode is
unavailable on your account or host. See the
[Antigravity CLI mode requirements](https://antigravity.google/docs/cli/modes#requirements-and-limitations) and
[supported agents and permission modes](/docs/agents/).

## Workspace trust

Antigravity asks whether to trust a directory it doesn't know. Farhelm shows the session as waiting while the question
is on screen. To skip the question when starting future Antigravity sessions, turn on **Skip Antigravity's workspace
trust question** under **Settings → Agents → Antigravity**. It applies both to directories you select and to managed
checkouts. The switch is off by default because trusting a directory lets its hooks, settings and commands run as if you
had approved them.

For that launch, the session's working directory replaces Antigravity's saved list of trusted directories. Farhelm
leaves your saved settings file alone. If you already pass `--settings` through **Custom** permission flags, that option
takes precedence. See [when Farhelm applies trust settings](/docs/using/start-a-session/#workspace-trust).

## Set up Restart

Turn on **Set up conversation hooks on all hosts** in **Settings → Agents → Antigravity**, or choose **Set up and
launch** when the session launcher offers it. The setting starts off. Once enabled, Farhelm installs or updates its
plugin at `~/.gemini/config/plugins/farhelm/` on each machine where it starts Antigravity, including inside a container
or sandbox. **Restart** becomes available after your first prompt, once Antigravity reports the conversation. Outside
Farhelm the hooks run and send Antigravity its expected empty reply.

A session started while the setting was off cannot be restarted, and a note above its terminal says so. Turn the setting
on and start a new session.

To remove the hooks, turn the setting off, delete that plugin directory from the machines where it was installed, and
start a new Antigravity process. If you previously set up hooks by hand, you can remove the old `farhelm` entry from
`~/.gemini/config/hooks.json` separately. You can open a conversation by ID with the setting off.

## Find a conversation ID

In the main Antigravity conversation, run `/help` and look for **Conversation** on the **General** tab. Enter that ID in
**Conversation ID**, under **More options** in the session launcher, when you want to open that conversation directly.
See [Stop, restart, and resume](/docs/using/stop-restart-resume/).

## Gotchas and quirks

- If **Restart** stays unavailable after your first prompt or opens the previous conversation, check `/hooks` in
  Antigravity. Then check the session's log at `<state dir>/hook-log/<session id>.log` (the state directory is usually
  `~/.local/state/farhelm`). [Agent hook injection](/docs/how-it-works/agent-hook-injection/#when-something-goes-wrong)
  explains its lines. You can also [find the conversation ID](/docs/agents/antigravity/#find-a-conversation-id) and open
  it directly.
- When Antigravity reports a conversation, Farhelm checks that the first line of its log records user input. This keeps
  subagent reports from changing **Restart**. The check was verified with Antigravity CLI 1.5.13. A missing or
  unreadable log or a first line over 64 KiB can leave **Restart** on the previous conversation or unavailable. A later
  report can establish the main conversation once its log can be read. A CLI update that changes the format can make
  Farhelm ignore every new report without a notification.
- Farhelm shows basic running and idle status by watching whether the screen changes. The workspace trust question shows
  as waiting in the session list. For approvals during a conversation, read and answer them in the terminal.
- If you start `agy` through a script, see [Agent wrappers](/docs/agents/agent-wrappers/) for what the script must pass
  on.
- Antigravity's conversation hooks do not introduce `$farhelm` requests.
