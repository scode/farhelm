# Farhelm plan

NOTE: This is the overall build plan: the motivation for how the work is
ordered, and the coarse milestone ladder. Only the current milestone is
ever planned in detail — see PLAN_M1.md. Later milestones get their own
PLAN_M*.md when their turn comes; pre-emptive detail would just be fiction
that dogfooding invalidates.

## How this gets built

In steps that are each end-to-end usable and testable. Not bottom-up: the
first milestone is a minimal product that works for real across the whole
stack, not a foundation layer. The failure mode this avoids is building
the bottom abstraction, layering upward, and discovering at the end that
the whole thing doesn't actually work — or doesn't feel right to use. An
end-to-end skeleton puts the riskiest integrations into contact with
reality first, and real daily use starts generating the true backlog
early.

Consequences of that stance:

- Every milestone ends usable and tested; there is no milestone whose
  deliverable is "infrastructure".
- Each milestone is built as thin verticals of the permanent crates
  (SPEC_impl.md's layout) — minimal scope, never prototype code with a
  planned rewrite.
- Dogfooding starts as soon as sessions are a managed thing (M2) and is
  expected to reorder everything after it. The ladder below is a map, not
  a contract.

## Milestone ladder

- **M0 — bootstrap.** Buildable workspace skeleton with the standing
  conventions applied before any product code: CI (fmt/clippy/test as
  separate jobs on Ubicloud), dprint, AGENTS.md with Conventional Commits
  and CI-matching finish-work commands, stacked-PR base guard. Planned in
  PLAN_M0.md.
- **M1 — walking skeleton.** One remote session end to end: real helm,
  protocol, supervisor, tmux, xterm.js; reconnect with replay; fake agent
  + Playwright harness; web and desktop UI from one crate. Planned in
  detail in PLAN_M1.md.
- **M2 — sessions as a managed thing.** Multiple sessions, the flat list,
  create/open/stop/delete from the GUI, supervisor SQLite, helm session
  cache. Dogfooding starts here.
- **M3 — durability and resume.** Supervisor-restart survival, boot-id
  interrupted classification, error/exited via the launch shim,
  conversation capture for Claude Code and Codex, restart-with-resume.
- **M4 — attachments and terminal tabs.** Paste/drop to path-at-cursor;
  tabs in the session cwd.
- **M5 — status and profiles.** Running/waiting/idle heuristics with
  per-agent sharpening, list filtering, profile CRUD and starter profiles.
- **M6 — multi-host.** Registry and host management, local-host
  supervisor, stale-cache semantics, version-skew refusal. Deliberately
  late: M1's argv-specified single host carries dogfooding a long way, and
  the registry is bookkeeping, not risk.
- **M7 — the outer ring.** Web-token auth and device sessions,
  `farhelm spawn` and agent-spawned sessions (deliberately late as well),
  archive, provisioning, Mac app bundling.
