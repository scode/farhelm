---
title: OMP
description: "What is specific to running OMP (oh-my-pi) under Farhelm."
sidebar:
  order: 10
---

The [support matrix](/docs/agents/) is the main reference for what Farhelm does with OMP. This page only adds what is
specific to OMP.

## Gotchas and quirks

- OMP runs without approval prompts by default, so an OMP launch with no permission choice counts as YOLO, and Farhelm
  asks you to confirm it on hosts that ask before YOLO launches.
- After `/new`, the new conversation can be restarted right away, before OMP has replied to it.
- OMP matches model names loosely: a typo or a retired model name can quietly start a different model instead of
  failing. If the exact model matters, check it in OMP itself after launch.
- Farhelm can follow the conversation when OMP is started directly, through `bun x` or `npx`, or as OMP's compiled
  binary. Running it under Node, or through some other wrapper, works normally, but **restart** stays greyed out.
- Restart refuses, rather than starting fresh, when OMP has compressed the conversation's file into a `.jsonl.gz`
  archive, or has moved it and not yet said where. Replace the session to start over.
