---
title: Cursor
description: "What is specific to running Cursor's cursor-agent CLI under Farhelm."
sidebar:
  order: 10
---

The [support matrix](/docs/agents/) is the main reference for what Farhelm does with Cursor. This page only adds what is
specific to Cursor.

## Setup

Install and sign in to the [Cursor CLI](https://cursor.com/docs/cli/overview) on every host you run it on. Farhelm
starts it as `cursor-agent`, so that name must be on the host's `PATH`. Cursor also installs itself as `agent`, but
Farhelm does not use that name, because other tools install a command called `agent` too.

## Gotchas and quirks

- **Agent's own setting** leaves permission to Cursor's configuration: `--auto-review` does not override a configured
  Run Everything mode. Farhelm shows a grey dashed shield and treats this launch as YOLO on hosts that ask first. See
  [Permission modes](/docs/agents/#permission-modes).
- Cursor asks whether you trust a directory the first time it starts there, also in YOLO mode. Answer it in the
  session's terminal; Cursor remembers your answer for that directory. (Seen with Cursor CLI 2026.09.18.)
- YOLO starts Cursor with `--force`, which still refuses commands your Cursor configuration explicitly denies.
- There is no effort choice. Pick a model variant, or put Cursor's own parameters in a custom model name, instead.
- Restart is not available, so to get back to an earlier conversation, use Cursor's own conversation picker in the
  terminal.
