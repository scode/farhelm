---
title: OMP
description: How Farhelm tracks OMP conversations through a private extension, and what it deliberately does not do.
---

OMP is the `omp` program, the `@oh-my-pi/pi-coding-agent` CLI — a fork of Pi, and Farhelm's integration mirrors that: a
private extension Farhelm supplies at launch reports which conversation you are in, and Farhelm does not scan OMP's own
state directory to guess at anything it was not told. The one bounded exception: before a resume, Farhelm reads a
bounded prefix of the exact session file the report named — wherever it lives, usually inside OMP's state directory — to
verify it still belongs to that conversation. This page describes the integration as built against OMP 18.2.4 and
18.2.6, both pinned at the source with the same rules for which launches count. Live lifecycle evidence was collected
against 18.2.6.

## What the integration deliberately does not do

Farhelm performs no OMP waiting-state recognition. When OMP shows an approval prompt, the session's status stays the
ordinary running/idle classification; it is never shown as waiting. This is a settled scope decision, not a detection
gap — approve or answer prompts in OMP's own terminal, not by watching the sidebar.

## Resume from your perspective

Farhelm offers Resume for the conversation the extension last reported. Typing `/new` starts a conversation that OMP
18.2.4 persists immediately, so the fresh conversation can be resumable right away instead of waiting for a first
assistant message.

Farhelm checks the reported session file before resuming: it reads a bounded prefix of the file without following
symlinks and requires the session header inside it to match the conversation it was told about. If the file is missing,
belongs to a different conversation, or does not read as the session it was reported to be, Farhelm refuses Resume and
offers a fresh launch instead — it never silently starts a new conversation under a Resume request. This matters because
OMP itself can silently start a new conversation when given a missing session file.

## How Farhelm knows the report is yours

The extension reports only from the interactive window. Background tasks and workpool children stay silent. A separate
interactive child must also pass process attribution: Farhelm checks the reporting process against the owned pane and
the launch's recorded program, rather than trusting inherited reporting credentials.

The launch record must name the current gated extension, whose installed bytes are checked before admission. Old
launches without that provenance remain runnable but cannot capture a conversation until relaunched. Supported process
chains include the Bun runtime running OMP's bundle or source entry, its compiled binary, the recognized `bun x` and
`npx` launchers, and transparent shell trampolines. Nested runtimes, Node running OMP, and unknown wrappers are refused.
A refused report leaves the saved identity unchanged; it cannot replace the root conversation's Resume target.

Live lifecycle evidence covers the Bun-executed entry. Compiled and package-launcher forms have process-chain shape
tests, but have not been exercised through the same live lifecycle scenarios.

## How your model id is resolved

You can leave the model unset in Farhelm. OMP then uses its configured provider and model, without a Farhelm
`--provider` or `--model` override. The rules below apply when you choose a model explicitly.

Farhelm stores the model id you enter verbatim and emits the command shown below, with `<id>` preserved as one argv
element. What Farhelm does not do is guarantee that OMP hands that exact string to OpenRouter: OMP's own model
resolution runs after Farhelm's argv, and it is deliberately fuzzy. OMP resolves provider-qualified ids through an exact
catalog match first, then falls back through alias and variant spellings, and finally to a provider-scoped fuzzy match —
so a typo or a retired id can land on a different model rather than failing. A trailing `:suffix` on an unknown id can
also be interpreted as a thinking level rather than part of the model name (OMP guards the common cases, but the
interpretation is OMP's, not Farhelm's). The explicit `--provider openrouter` spelling makes provider intent explicit;
it does not switch that resolution off. If exact upstream routing of an arbitrary custom id matters, verify it in OMP
itself: the composer accepts a syntactically valid custom id that need not exist in OMP's catalog, subject to Farhelm's
existing harness-compatibility checks on which harness owns which id.

## Limitations

- OMP can move an active conversation's file without emitting any event Farhelm subscribes to. Until the next event
  reports the new path, the resume offer may point at the old location; the pre-resume check is what keeps a stale offer
  from resuming a file that is no longer there.
- A conversation stored somewhere other than a session file gets no resume offer: the report still arrives, but with no
  file to verify, and the offer withdraws.
- A session file OMP has compressed into a `.jsonl.gz` archive cannot be verified, so its resume offer withdraws the
  same way — fail closed, never a silent fresh start.
- A session file with no title slot in front of its header cannot be told apart from a Pi-shaped file by its bytes, so
  Farhelm tells OMP and Pi apart by the reports and locators they exchange, not by file contents.

The extension Farhelm loads with `-e` is supplied per launch and is not saved with the conversation. Reopening an OMP
conversation outside Farhelm does not require Farhelm to be installed.
