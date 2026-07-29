# Farhelm plan

NOTE: This is the overall build plan: the motivation for how the work is ordered, and the coarse milestone ladder. Only
the current milestone is ever planned in detail — see PLAN_M3.md (PLAN_M0.md through PLAN_M2_5.md are history). Later
milestones get their own PLAN_M*.md when their turn comes; pre-emptive detail would just be fiction that dogfooding
invalidates.

## How this gets built

In steps that are each end-to-end usable and testable. Not bottom-up: the first milestone is a minimal product that
works for real across the whole stack, not a foundation layer. The failure mode this avoids is building the bottom
abstraction, layering upward, and discovering at the end that the whole thing doesn't actually work — or doesn't feel
right to use. An end-to-end skeleton puts the riskiest integrations into contact with reality first, and real daily use
starts generating the true backlog early.

Consequences of that stance:

- Every milestone ends usable and tested; there is no milestone whose deliverable is "infrastructure".
- Each milestone is built as thin verticals of the permanent crates (SPEC_impl.md's layout) — minimal scope, never
  prototype code with a planned rewrite.
- Dogfooding starts as soon as sessions are a managed thing (M2) and is expected to reorder everything after it. The
  ladder below is a map, not a contract.

## Milestone ladder

- **M0 — bootstrap.** Buildable workspace skeleton with the standing conventions applied before any product code: CI
  (fmt/clippy/test as separate jobs on Ubicloud), dprint, AGENTS.md with Conventional Commits and CI-matching
  finish-work commands, stacked-PR base guard. Planned in PLAN_M0.md.
- **M1 — walking skeleton.** One remote session end to end: real helm, protocol, supervisor, tmux, xterm.js; reconnect
  with replay; fake agent and Playwright harness; web and desktop UI from one crate. Planned in detail in PLAN_M1.md.
- **M2 — sessions as a managed thing.** Multiple sessions, the flat list, create/open/stop/delete from the GUI,
  supervisor SQLite. Also the protocol growth the list forces: a hard cap with total count and truncated flag on the
  session-list response (today it is one frame, defused to a per-request error when oversize), and widening the protocol
  error taxonomy as the GUI's error surfacing demands it. The list is poll-refreshed in M2; live push is M5's. Real
  cursor pagination of the list is deferred to M6 with the cap standing in. Dogfooding starts here. Planned in detail in
  PLAN_M2.md.
- **M2.5 — terminal-path backpressure.** The end-to-end flow control SPEC_impl.md already specifies: `term.write()`
  completion callbacks drive watermark pause/resume over the WebSocket, the supervisor throttles its pane reads, and the
  internal queues become bounded. Deferred out of M1 (the review flagged the unbounded queues); scheduled right after
  dogfooding starts because sustained heavy agent output is what turns the risk real.
- **M3 — durability and resume.** Supervisor-restart survival, boot-id interrupted classification, error/exited via the
  launch shim, conversation capture for Claude Code and Codex, restart-with-resume. Includes the crash-safety groundwork
  the M1 review deferred here: an explicit atomicity policy for state-file writes (temp-write-then-rename with fsync
  where a torn file would matter) and a fault-injection seam so the failure windows are testable, not just reasoned
  about. Also the process-tree ownership hardening SPEC_impl.md describes: `systemd-run --user --scope` cgroups on Linux
  where a user manager exists, layered over M2's /proc-walk-plus-env-marker sweep (which stays as the fallback). And
  server-enforced create idempotency: SPEC.md's "one intended create yields one session, never two silently" is only
  half-met by M2's client-side double-submit guard — a retry after an ambiguous transport failure can still duplicate,
  and closing that needs a client-supplied intent key deduplicated in the supervisor's store, which is this milestone's
  durability territory. Also SPEC.md's durable stop annotations — no ladder entry ever claimed them (found while
  planning M3 in detail), and durable session metadata is exactly this milestone's ground.
- **M4 — attachments and terminal tabs.** Paste/drop to path-at-cursor; tabs in the session cwd.
- **M5 — status and profiles.** Running/waiting/idle heuristics with per-agent sharpening, list filtering, profile CRUD
  and starter profiles. Also live push of session-list changes to connected clients, replacing M2's polling — placed
  here because status transitions are what make polling genuinely painful, and the push channel serves both. Also
  session rename: PLAN_M2.md deferred it as "M3+" and no entry ever claimed it (a ladder gap found while planning M3);
  it lands here because it is a metadata-CRUD-plus-list-UX change of exactly this milestone's shape, not durability
  work.
- **M6 — multi-host.** Registry and host management, local-host supervisor, stale-cache semantics — including the
  helm-side persistent last-known session cache (helm.db) that SPEC.md's stale-list behavior needs, deferred out of M2
  where a single always-connected supervisor made it dead weight. Version-skew refusal. Real cursor pagination of the
  session list replaces M2's hard cap here, when multi-host aggregation is what could actually grow lists past it.
  Deliberately late: M1's argv-specified single host carries dogfooding a long way, and the registry is bookkeeping, not
  risk.
- **M6.5 — test-coverage backfill.** Coverage debt deliberately parked while the focus was end-to-end progress, tracked
  here so it cannot be quietly forgotten. Known entries: unit coverage for terminal.js's `onBinary` byte conversion,
  which needs a JS test-harness decision first (the repo has only Playwright today, and adopting a JS unit runner for
  one small function was judged premature during the M1 review); a fake-agent script that enables mouse modes on cue
  plus an e2e test pinning mouse-mode restoration on reattach (the restoration code shipped in M1 untested — PaneModes
  captures the modes, but nothing exercises them end to end); and a reusable drive-a-real-agent Playwright helper that
  bakes in the lessons from the first agent-driven smoke test (Claude Code's trust dialog, its fast-typing paste
  heuristic swallowing Enter, reply-marker detection). Placed late because the parked items are small, stable code with
  low regression risk; anything that starts changing often should be pulled forward instead of waiting here.
- **M7 — the outer ring.** Web-token auth and device sessions, `farhelm spawn` and agent-spawned sessions (deliberately
  late as well), archive, provisioning, Mac app bundling.
