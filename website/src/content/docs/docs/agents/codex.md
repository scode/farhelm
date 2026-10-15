---
title: Codex
description: "What is specific to running Codex under Farhelm."
sidebar:
  order: 10
---

The [support matrix](/docs/agents/) is the main reference for what Farhelm does with Codex. This page only adds what is
specific to Codex.

## Gotchas and quirks

- Guarded passes `-a on-request -s workspace-write`, so Codex works within its workspace sandbox and requests approval
  to step outside it. Your configured reviewer can decide those requests, and your sandbox settings or later mode
  switches can loosen the mode. See [Permission modes](/docs/agents/#permission-modes).
- Codex shows `⚠ --dangerously-bypass-hook-trust is enabled` above its composer in sessions Farhelm launches with its
  conversation hook, the hook that lets **restart** resume the conversation you were in. The same bypass also lets your
  own Codex hooks, including a trusted workspace's, run without Codex's review. Turning it off means those Codex
  sessions can't be restarted. Read [what the warning means](/docs/how-it-works/agent-hook-injection/#what-you-will-see)
  and [how to turn it off](/docs/how-it-works/agent-hook-injection/#turning-it-off).
- **restart** becomes available only after you submit your first prompt. A Codex session you never prompted cannot be
  restarted; replace it instead.
- Workspace trust in the session launcher applies to that one launch only: **true** trusts the session's directory and
  **false** marks it untrusted, without changing what Codex has saved about that directory.
- After `/clear`, restart no longer resumes the old conversation, and it resumes the new one once Codex has finished its
  next reply.
- If you start Codex through a launcher of your own (a [custom command](/docs/agents/custom-commands/)), Farhelm can
  follow its conversation even when the launcher stays running as Codex's parent. It must forward Farhelm's arguments
  and environment, and the native Codex executable must still be named `codex`. Put the launcher in the resume command
  too, so **restart** goes through it. [Agent wrappers](/docs/agents/agent-wrappers/) has the details.
- On a remote host, Codex's browser sign-in reports that it opened the link before you answer the card, so it cannot
  tell when you deny it. A sign-in that redirects back to `localhost` on the host still cannot finish from your browser.
