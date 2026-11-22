---
title: Pi
description: "What is specific to running Pi under Farhelm."
sidebar:
  order: 10
---

The [support matrix](/docs/agents/) is the main reference for what Farhelm does with Pi. This page only adds what is
specific to Pi.

## Gotchas and quirks

- Pi never asks before running commands or changing files, which is why YOLO is its only permission choice. Its switch
  in [agent trust settings](/docs/using/start-a-session/#workspace-trust) approves the project's settings and extensions
  in any directory. Farhelm passes `--approve` for the launch and Pi keeps its saved configuration. Approved resources
  can run commands.
- **Restart** becomes available once Pi has saved the conversation, after its first reply. After `/new`, restart no
  longer resumes the old conversation, and resumes the new one once Pi has saved it.
- Restart refuses, rather than starting fresh, when the conversation's file has gone missing or now holds a different
  conversation. Replace the session to start over.
