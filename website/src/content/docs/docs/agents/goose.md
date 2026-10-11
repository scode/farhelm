---
title: Goose
description: "What is specific to running Goose under Farhelm."
sidebar:
  order: 10
---

The [support matrix](/docs/agents/) is the main reference for what Farhelm does with Goose. This page only adds what is
specific to Goose.

## Gotchas and quirks

- Goose runs without approval prompts unless you have set it to ask, so a Goose launch with no permission choice counts
  as YOLO, and Farhelm asks you to confirm it on hosts that ask before YOLO launches.
- If Goose runs subagents, a subagent's conversation can replace yours as the one restart resumes.
- Reopening a Goose conversation directly in Goose, outside Farhelm, needs the `farhelm` command on your `PATH`. Goose
  saves the small helper Farhelm adds to each conversation and starts it again on every resume; without `farhelm` it
  reports an extension error. Outside Farhelm the helper does nothing.
- A [custom command](/docs/agents/custom-commands/) that runs Goose must start an interactive session itself, as
  `goose session {farhelm_args}`, with the resume command
  `goose session --resume --session-id {conversation} {farhelm_args}`.
