---
title: Muse
description: "What is specific to running Muse under Farhelm."
sidebar:
  order: 10
---

The [support matrix](/docs/agents/) is the main reference for what Farhelm does with Muse. This page only adds what is
specific to Muse.

## Gotchas and quirks

- Guarded selects Muse's auto-review profile with `--permission-profile :auto-review`, overriding an unrestricted
  default. Your approval rules, sandbox settings and later mode switches still matter; see
  [Permission modes](/docs/agents/#permission-modes). Farhelm also tries to notice **YOLO** on Muse's status line and
  show a YOLO shield; this is best effort, not a check of your approval or sandbox settings.
- YOLO also makes Muse trust the session's directory, so choosing **false** for workspace trust changes nothing on a
  YOLO launch, or when Muse's own settings already trust the directory.
