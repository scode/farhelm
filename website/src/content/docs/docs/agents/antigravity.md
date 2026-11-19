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

## Gotchas and quirks

- If **Restart** never becomes available after your first prompt, the hooks are probably not loaded: check `/hooks` in
  Antigravity, then the session's log at `<state dir>/hook-log/<session id>.log` (the state directory is usually
  `~/.local/state/farhelm`); [Agent hook injection](/docs/how-it-works/agent-hook-injection/#when-something-goes-wrong)
  explains its lines.
- Antigravity CLI 1.5.12 sends the same hook events for its main conversation and subagent conversations in the same
  process. **Restart** can resume a subagent's conversation if that subagent reported last, until the main conversation
  reports again. To open the main conversation directly, enter its ID in the launcher's **Conversation ID** field. See
  [Stop, restart, and resume](/docs/using/stop-restart-resume/).
- Farhelm shows basic running and idle status by watching whether the screen changes. Antigravity's approval requests
  don't show as waiting in the session list. Read and answer them in the session's terminal.
- Remove the `farhelm` entry from the hooks file and start a new Antigravity process to turn off reports. The settings
  that control the hooks Farhelm adds to other agents have no effect on these entries.
- If you start `agy` through a script, see [Agent wrappers](/docs/agents/agent-wrappers/) for what the script must pass
  on.
- The hooks do nothing when `agy` runs outside Farhelm, apart from the empty reply Antigravity expects.
- Antigravity is not told about `$farhelm` requests, because Farhelm's hooks for it are the ones you install.
