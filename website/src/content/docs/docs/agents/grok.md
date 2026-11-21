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

Turn on **Set up conversation hooks on all hosts** in **Settings → Agents → Grok**, or choose **Set up and launch** when
the session launcher offers it. The setting starts off. Once enabled, Farhelm installs or updates
`~/.grok/hooks/farhelm.json` on each machine where it starts Grok, including inside a container or sandbox. The hooks
let **Restart** resume the conversation after your first prompt. The hooks also run when you start Grok outside Farhelm,
where they exit quietly.

If the supervisor has `GROK_HOME` set in its environment, Farhelm uses `$GROK_HOME/hooks/farhelm.json` instead. Setting
`GROK_HOME` only in a shell startup file does not reach the supervisor. To remove the hooks, turn the setting off,
delete the file from the machines where it was installed, and start a new Grok process.

## Gotchas and quirks

- **Restart** becomes available after your first prompt. If it does not, the hooks are probably not loaded: check
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
- Grok works through a [wrapper prefix](/docs/agents/agent-wrappers/) that `exec`s it or stays as its one parent
  process, as long as the wrapper passes through its arguments and environment. A deeper chain of resident wrappers, a
  package-manager launcher, or a renamed binary runs it normally, but Farhelm cannot follow its conversation, so
  **Restart** stays greyed out. So does a resident wrapper that is itself named `grok`, or whose program is named
  `grok-` followed by anything.
- Grok's conversation hooks do not introduce `$farhelm` requests.
- Tested on Linux with Grok Build 1.0.40 and 1.0.50; not yet on a Mac.
