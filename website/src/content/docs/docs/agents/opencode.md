---
title: OpenCode
description: "What is specific to running OpenCode under Farhelm."
sidebar:
  order: 10
---

The [support matrix](/docs/agents/) is the main reference for what Farhelm does with OpenCode. This page only adds what
is specific to OpenCode.

## Gotchas and quirks

- Only models from OpenCode's own provider can be chosen. A model name without a provider, such as `grok-4.6`, is passed
  to OpenCode as `opencode/grok-4.6`, and a name that starts with any other provider is refused.
