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
  in [managed-checkout settings](/docs/using/start-a-session/#trust-in-managed-checkouts) approves the project's own
  settings and extensions. It is off by default and applies across hosts: each new launch in a directory Farhelm manages
  gets `--approve`, without changing Pi's saved configuration. Those project resources may run commands as if you had
  approved them. Directories outside managed checkouts keep Pi's own project-approval behavior; this setting does not
  add approval prompts for the agent's actions.
- **restart** becomes available once Pi has saved the conversation, after its first reply. After `/new`, restart no
  longer resumes the old conversation, and resumes the new one once Pi has saved it.
- Restart refuses, rather than starting fresh, when the conversation's file has gone missing or now holds a different
  conversation. Replace the session to start over.
