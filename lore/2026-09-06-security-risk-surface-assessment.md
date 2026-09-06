# Security and correctness risk surface assessment

NOTE: this is a survey done to decide where to spend review effort, not an audit and not a decision. Nothing in it
asserts a defect, and no path it names was verified for correctness. It ranks areas by what a defect there would cost,
not by evidence that one exists.

Done on 2026-09-06 at `ded1596b` by Claude Opus 5 (1M context), in one session, reading the trust boundaries and the
largest modules directly rather than delegating. The full ranked write-up lives in a Claude artifact:

https://claude.ai/code/artifact/e4595b5d-2421-4ea9-b0c9-887eca4d0702

The prompt that produced it, verbatim:

> Assess the overall codebase and identify the most security- and correctness-critical areas that need the most
> attention in terms of code review and architectural assessment. produce a list of top 10 areas in ranked order.

## What the ranking came out as

In order: the agent upcall relay and what a per-session credential authorizes; the release supply chain and remote
provisioning; the process kill sweep; the helm's HTTP and WebSocket authentication edge; the supervisor core; schema
migration across both stores; tmux control-mode parsing and streaming; the attachment upload path end to end; the
desktop shell; and untrusted text reaching the DOM and the eval'd JavaScript.

## The one thing worth reading even if the artifact goes stale

Two observations drove the ordering, and both are about the shape of the system rather than about any particular file.

The first is that `AgentVerb::Create` takes a raw invocation, an arbitrary cwd, and a target host named by display name,
authorized by a credential that lives in the session's environment and is therefore inherited by every subprocess the
agent spawns. That is arbitrary command execution on every registered machine, reachable by prompt injection into an
agent whose whole job is reading untrusted content. The relay's own module docs argue the fleet-wide authority is
deliberate, and that argument holds for rename/stop/archive — inventing a narrower permission model for agents alone
would be a second authorization scheme with nothing to keep it honest. The raw-invocation half is the part I would
re-decide rather than inherit.

The second is a calibration that changes how the rest of the list should be read. The security hygiene here is already
above average and, unusually, written down: `shell_words` at every SSH boundary, argv-element substitution rather than
string templating for `{conversation}` and `{cwd}`, `serde_json` rather than interpolation at every `document::eval`,
release checksums whose signature binds the version in its trusted comment, OSC 52 read refused unconditionally, the
supervisor socket at 0600 inside a 0700 directory. So the ranking is weighted toward blast radius and unsettled
structural questions, not toward anything that looks careless. Several entries are ranked where they are *because* the
existing discipline holds — which is also the argument for making that discipline mechanical instead of remembered.
