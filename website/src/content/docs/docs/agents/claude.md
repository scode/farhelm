---
title: Claude
description: "What is specific to running Claude Code under Farhelm."
sidebar:
  order: 10
---

The [support matrix](/docs/agents/) is the main reference for what Farhelm does with Claude. This page only adds what is
specific to Claude.

## Gotchas and quirks

- Claude may ask whether you trust a directory the first time it starts there. Answer it in the session's terminal; the
  session launcher has no way to answer it for you, and Claude remembers your answer for that directory.
- If you start Claude through a script or wrapper of your own (a [custom command](/docs/agents/custom-commands/)),
  Farhelm can follow its conversation, and so restart it, only when Claude is the program the session started or one
  level below it. A deeper chain of wrappers still runs Claude normally, but **restart** stays greyed out.
  [Agent wrappers](/docs/agents/agent-wrappers/) shows how to write a wrapper that keeps tracking working.
- On a Linux host, Claude's own way of opening links does not reach your browser through Farhelm. Ask it to run
  `farhelm agent open-url -- <url>` instead
  ([Open links from a remote session](/docs/using/approve-agent-requests/#open-links-from-a-remote-session)).
