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
- To skip Codex's directory-trust question in
  [managed checkouts](/docs/using/start-a-session/#trust-in-managed-checkouts), enable its switch under **agent trust**
  in **settings**. It is off by default and applies across hosts when Farhelm starts a new launch in a directory it
  manages. Farhelm passes a trust argument for that launch without changing Codex's saved configuration. Trust lets the
  repository's own hooks, settings and commands run as if you had approved them. Directories outside managed checkouts
  keep Codex's own trust behavior.
- **restart** resumes the conversation Farhelm last learned of. After `/clear`, Farhelm learns of the new conversation
  when you send your next prompt, and restart can resume it once Codex has finished that reply. Restarting before you
  send that prompt can bring back the earlier conversation.
- If you start Codex through a [wrapper prefix](/docs/agents/agent-wrappers/) or a
  [custom command](/docs/agents/custom-commands/), Farhelm can follow its conversation even when the launcher stays
  running as Codex's parent. It must forward Farhelm's arguments and environment, and the native Codex executable must
  still be named `codex`. Put the launcher in the resume command too for a custom command, so **restart** goes through
  it. Farhelm includes a prefix in the resume command itself.
- On a remote host, Codex's browser sign-in reports that it opened the link before you answer the card, so it cannot
  tell when you deny it. A sign-in that redirects back to `localhost` on the host still cannot finish from your browser.
