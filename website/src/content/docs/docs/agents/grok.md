---
title: Grok
description: "What is specific to running the Grok Build CLI under Farhelm, including the hooks it needs."
sidebar:
  order: 10
---

The [support matrix](/docs/agents/) is the main reference for what Farhelm does with Grok. This page only adds what is
specific to Grok.

## Setup

Install and sign in to `grok` on every host you run it on. Farhelm starts it as `grok --no-leader`, which keeps Grok's
backend inside the session; that costs some backend reuse and memory, and native subagents still work.

Farhelm can follow a Grok conversation, and so restart it, only after you add three hooks to Grok's configuration
yourself. Farhelm does not edit Grok's configuration. On every host, create `$GROK_HOME/hooks/farhelm.json` (or
`~/.grok/hooks/farhelm.json` when `GROK_HOME` is unset), replacing `/absolute/path/to/farhelm` with the path of the
`farhelm` command on that host:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          { "type": "command", "command": "/absolute/path/to/farhelm internal hook --vendor grok", "timeout": 60 }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          { "type": "command", "command": "/absolute/path/to/farhelm internal hook --vendor grok", "timeout": 60 }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          { "type": "command", "command": "/absolute/path/to/farhelm internal hook --vendor grok", "timeout": 60 }
        ]
      }
    ]
  }
}
```

Grok merges every file in its hooks directory, so leave your other hook files alone. Restart Grok, and check with its
`/hooks` view that all three entries loaded. To remove the integration, delete the file (or these three entries) and
restart Grok. The hooks do nothing for a Grok you start outside Farhelm.

## Gotchas and quirks

- **restart** becomes available after your first prompt. If it does not, the hooks are probably not loaded: check
  `/hooks` in Grok, then the session's log at `<state dir>/hook-log/<session id>.log` (the state directory is usually
  `~/.local/state/farhelm`); [Agent hook injection](/docs/how-it-works/agent-hook-injection/#when-something-goes-wrong)
  explains its lines.
- Farhelm follows one top-level conversation per session. `/new` is followed; `/fork`, several top-level conversations
  in one session, and switching between them are not. Use a separate session for each conversation you want to keep.
- If Grok exits right after `/new`, restart can resume the conversation before it.
- Guarded starts Grok with `--permission-mode auto`, selecting auto-review even when its configured default is
  always-approve. Your own allow rules and later mode switches can still loosen it; see
  [Permission modes](/docs/agents/#permission-modes). Farhelm also tries to notice **always-approve** on Grok's status
  line and show a YOLO shield; this is best effort, not a check of your allow rules.
- Starting Grok through a wrapper, a package-manager launcher, or a renamed binary runs it normally, but Farhelm cannot
  follow its conversation, so **restart** stays greyed out.
- Grok is not told about `$farhelm` requests, because Farhelm's hooks for it are the ones you install.
- Tested on Linux with Grok Build 1.0.40 and 1.0.50; not yet on a Mac.
