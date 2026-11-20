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

## Set up Restart

Install two [Antigravity CLI hooks](https://antigravity.google/docs/hooks#managing-hooks-in-antigravity-cli) as the user
who runs `agy`. For a container or sandbox session, set them up inside that environment. Run `command -v farhelm` there
to get the full path of the `farhelm` command. Use that path in place of `/absolute/path/to/farhelm` below. The single
quotes around the path keep spaces in it together.

Create `~/.gemini/config/hooks.json` with this content. If you already have that file, add the `farhelm` entry alongside
your other entries.

```json
{
  "farhelm": {
    "SessionStart": [
      { "type": "command", "command": "'/absolute/path/to/farhelm' internal hook --vendor antigravity", "timeout": 60 }
    ],
    "Stop": [
      { "type": "command", "command": "'/absolute/path/to/farhelm' internal hook --vendor antigravity", "timeout": 60 }
    ]
  }
}
```

Start a new Antigravity session and run `/hooks` in it to check that both entries loaded. **Restart** becomes available
after your first prompt, once Antigravity reports the conversation. You can still launch and explicitly open a
conversation by ID without installing the hooks.

## Find a conversation ID

In the main Antigravity conversation, run `/help` and look for **Conversation** on the **General** tab. Enter that ID in
the session launcher's **Conversation ID** field when you want to open that conversation directly. See
[Stop, restart, and resume](/docs/using/stop-restart-resume/).

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
- Farhelm shows basic running and idle status by watching whether the screen changes. Antigravity's approval requests
  don't show as waiting in the session list. Read and answer them in the session's terminal.
- Remove the `farhelm` entry from the hooks file and start a new Antigravity process to turn off reports. The settings
  that control the hooks Farhelm adds to other agents have no effect on these entries.
- If you start `agy` through a script, see [Agent wrappers](/docs/agents/agent-wrappers/) for what the script must pass
  on.
- The hooks do nothing when `agy` runs outside Farhelm, apart from the empty reply Antigravity expects.
- Antigravity is not told about `$farhelm` requests, because Farhelm's hooks for it are the ones you install.
