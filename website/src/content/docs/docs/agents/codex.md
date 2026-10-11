---
title: Codex
description: "What is specific to running Codex under Farhelm."
sidebar:
  order: 10
---

The [support matrix](/docs/agents/) is the main reference for what Farhelm does with Codex. This page only adds what is
specific to Codex.

## Gotchas and quirks

- Codex prints a warning about a hook at the start of every session Farhelm launches. That hook is how Farhelm learns
  which conversation you are in, and the warning is harmless.
- **restart** becomes available only after you submit your first prompt. A Codex session you never prompted cannot be
  restarted; replace it instead.
- Workspace trust in the session launcher applies to that one launch only: **true** trusts the session's directory and
  **false** marks it untrusted, without changing what Codex has saved about that directory.
- After `/clear`, restart no longer resumes the old conversation, and it resumes the new one once Codex has finished its
  next reply.
- If you start Codex through a launcher of your own (a [custom command](/docs/agents/custom-commands/)), Farhelm can
  follow its conversation only when that launcher replaces itself with `codex` (`exec codex …`) or is a plain shell
  command. A Node launcher, a shell script that stays running, or a renamed `codex` binary still runs Codex normally,
  but **restart** stays greyed out. Give such a command the resume command `codex resume {conversation} {farhelm_args}`.
  [Agent wrappers](/docs/agents/agent-wrappers/) has the details.
