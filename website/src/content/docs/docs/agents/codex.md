---
title: Codex
description: "What is specific to running Codex under Farhelm."
sidebar:
  order: 10
---

The [support matrix](/docs/agents/) is the main reference for what Farhelm does with Codex. This page only adds what is
specific to Codex.

## Gotchas and quirks

- Farhelm passes `--no-daemon` so Codex runs its commands with your session's own environment and processes, even when
  its conversation hook is off. It no longer shows Codex's "Running without the shared background server" warning. This
  needs Codex 0.156.0 or later. A custom Codex command that puts `{farhelm_args}` after `exec`, passes `--remote`, or
  runs `codex agents` or `codex queue` fails because those commands cannot take this flag. Sessions started before
  Farhelm 0.22 from a typed command or profile retain their old behavior; recreating one through **Replace with** gives
  it the current launch arguments.
- Guarded passes `-a on-request -s workspace-write`, so Codex works within its workspace sandbox and requests approval
  to step outside it. Your configured reviewer can decide those requests, and your sandbox settings or later mode
  switches can loosen the mode. See [Permission modes](/docs/agents/#permission-modes).
- Codex shows `⚠ --dangerously-bypass-hook-trust is enabled` above its composer in sessions Farhelm launches with its
  conversation hook, the hook that lets **Restart** resume the conversation you were in. The same bypass also lets your
  own Codex hooks, including a trusted workspace's, run without Codex's review. Turning it off means those Codex
  sessions can't be restarted. Read [what the warning means](/docs/how-it-works/agent-hook-injection/#what-you-will-see)
  and [how to turn it off](/docs/how-it-works/agent-hook-injection/#turning-it-off).
- **Restart** becomes available only after you submit your first prompt. A Codex session you never prompted cannot be
  restarted; replace it instead.
- On installed hosts, **Answer Codex's folder-trust question** under **Settings → Agents → Codex** skips the question.
  The switch applies in any directory. Farhelm passes a trust argument for the launch and Codex keeps its saved
  configuration. See [workspace trust](/docs/using/start-a-session/#workspace-trust) for when the switch and the note
  above the terminal apply.
- **Restart** resumes the conversation Farhelm last learned of. After `/clear`, Farhelm learns of the new conversation
  when you send your next prompt, and restart can resume it once Codex has finished that reply. Restarting before you
  send that prompt can bring back the earlier conversation.
- If you start Codex through a [wrapper prefix](/docs/agents/agent-wrappers/) or a
  [custom command](/docs/agents/custom-commands/), Farhelm can follow its conversation even when the launcher stays
  running as Codex's parent or the executable has been renamed. Forward Farhelm's arguments and environment. Put the
  launcher in the resume command too for a custom command, so **Restart** goes through it. Farhelm includes a prefix in
  the resume command itself.
- On a remote host, Codex's browser sign-in reports that it opened the link before you answer the card, so it cannot
  tell when you deny it. A sign-in that redirects back to `localhost` on the host still cannot finish from your browser.
