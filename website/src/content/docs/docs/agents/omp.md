---
title: OMP
description: "What is specific to running OMP (oh-my-pi) under Farhelm."
sidebar:
  order: 10
---

The [support matrix](/docs/agents/) is the main reference for what Farhelm does with OMP. This page only adds what is
specific to OMP.

## Gotchas and quirks

- Guarded uses `--approval-mode write`, automatically approving workspace edits and asking before other actions. The
  separate approve choice asks before every tool action. Project rules and plugins can loosen either; see
  [Permission modes](/docs/agents/#permission-modes).
- New OMP launches default to Guarded unless you chose another mode or the launcher remembered one. Farhelm does not
  recognize OMP's approval prompts: a session waiting for your approval still shows as running or idle, with no waiting
  alert.
- After `/new`, the new conversation can be restarted right away, before OMP has replied to it.
- OMP matches model names loosely: a typo or a retired model name can quietly start a different model instead of
  failing. If the exact model matters, check it in OMP itself after launch.
- Farhelm can follow the conversation when OMP is started directly, through `bun x` or `npx`, or as OMP's compiled
  binary. Running it under Node, or through some other wrapper, works normally, but **restart** stays greyed out.
- Restart refuses, rather than starting fresh, when OMP has compressed the conversation's file into a `.jsonl.gz`
  archive, or has moved it and not yet said where. Replace the session to start over.
