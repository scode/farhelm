# Farhelm plan

NOTE: This is the overall build plan: the motivation for how the work is ordered, and the coarse milestone ladder. Only
the current milestone is ever planned in detail — see PLAN_M7.md (PLAN_M0.md through PLAN_M6_75.md are history). Later
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
  error taxonomy as the GUI's error surfacing demands it. The list is poll-refreshed in M2; live push is M6.75's. Real
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
- **M4 — attachments and terminal tabs.** Paste/drop to path-at-cursor; tabs in the session cwd. Planned in detail in
  PLAN_M4.md.
- **M4.5 — structural refactor pass.** Functional no-ops only, recommended by the post-M4 architecture assessment and
  kept out of the functional stack deliberately: named per-message handlers replacing handle_control's giant match arms,
  a service/ module directory split, a directory-style e2e test layout, and a present-tense refresh of module headers
  that still narrate in milestone diffs. Each ships as its own refactor PR checked by the existing suite.
- **M5 — daily-driver polish.** The reordered ladder's fast path to primary use (2026-08-03: status/profiles moved to
  M6.75 so persistent multi-host use starts sooner — the user wants to daily-drive and let dogfooding find the bugs).
  Reattach replay presentation (found in post-M3 manual testing): reopening a session visibly re-scrolls the whole
  retained history, and the reattach cost grows with scrollback size; doing this right needs a replay-complete marker in
  the protocol so the client can batch or hide the prefill and land at the tail. The marker rides the terminal stream,
  so it does not need M6.75's list-push channel — the old ladder bundled them purely for protocol-work batching. Also
  session rename: PLAN_M2.md deferred it as "M3+" and no entry ever claimed it (a ladder gap found while planning M3); a
  small metadata-CRUD-plus-list-UX change that daily use will want early. Carried in with it: a confirmation pass for
  terminal selection dismissal on real WKWebView. A select-and-copy leaves BOTH an xterm selection and a native document
  selection over the DOM rows, and only the second one stayed painted through paste, typing, and a forced repaint; the
  fix that drops both is verified headlessly in Chromium and Playwright's WebKit but was still painting on the macOS
  desktop app when the manual round ran out of time, so the remaining suspect — WKWebView holding the selection layer
  after its ranges are gone — is unconfirmed either way. Planned in detail in PLAN_M5.md.
- **M6 — multi-host.** Registry and host management, local-host supervisor, stale-cache semantics — including the
  helm-side persistent last-known session cache (helm.db) that SPEC.md's stale-list behavior needs, deferred out of M2
  where a single always-connected supervisor made it dead weight. Version-skew refusal. Real cursor pagination of the
  session list replaces M2's hard cap here, when multi-host aggregation is what could actually grow lists past it.
  Deliberately late: M1's argv-specified single host carries dogfooding a long way, and the registry is bookkeeping, not
  risk. The terminal websocket's auto-reconnect also lands here (found in post-M3 manual testing: a laptop sleep kills
  only that socket, and today recovery is a manual back-and-reopen): SPEC.md's Errors section already assigns
  reconnection with bounded retries to the connection-state model this milestone builds, so a client-side quick fix
  beforehand would preempt the designed behavior. Planned in detail in PLAN_M6.md.
- **M6.5 — test-suite backfill.** Test debt deliberately parked while the focus was end-to-end progress, tracked here so
  it cannot be quietly forgotten. Known entries: unit coverage for terminal.js's `onBinary` byte conversion, which needs
  a JS test-harness decision first (the repo has only Playwright today, and adopting a JS unit runner for one small
  function was judged premature during the M1 review); a fake-agent script that enables mouse modes on cue plus an e2e
  test pinning mouse-mode restoration on reattach (the restoration code shipped in M1 untested — PaneModes captures the
  modes, but nothing exercises them end to end); and a reusable drive-a-real-agent Playwright helper that bakes in the
  lessons from the first agent-driven smoke test (Claude Code's trust dialog, its fast-typing paste heuristic swallowing
  Enter, reply-marker detection). Placed late because the parked items are small, stable code with low regression risk;
  anything that starts changing often should be pulled forward instead of waiting here. Also what remains of the CI
  one-off flake class first observed while closing M4 (2026-08-03) and investigated during the flake-fix series
  (2026-08-03): single distinct tests failing once per loaded run, passing on rerun and in isolation. The original "tmux
  timing" framing proved too narrow — the honest predicate is load-sensitive coupling between real-world events and a
  test's notion of time, with three sub-classes found and fixed differently: real tmux operations outrunning fixed
  budgets (fixed by making the supervisor's exchange budgets injectable and floored suite-wide, plus readiness waits
  that poll state rather than files or wall time); tokio paused-clock auto-advance burning virtual time while a test
  awaits a real-world event (fixed by keeping blocking calls off the runtime); and probe-verdict divergence between a
  test's precondition and the supervisor's own forever-cached probe (fixed by injecting the probed manager through the
  seam). Individually root-caused and fixed in that series: pause-spam detach, sink-backoff reset,
  scope-survives-restart, exited-agent-viewable, ack-ahead-of-backlog, orphaned-sink-client (alt-screen reattach fell
  earlier, during M5). Pane-silenced was diagnosis-hardened rather than closed: tmux's %exit reason now survives to the
  panic message, and its disappearance mechanism stays open — server-death-under-load is the leading hypothesis and the
  new diagnostics are the instrument for the next occurrence. Connect-names-socket was proven to have no internal
  failure mechanism (no timing constructs at all); its one-off must be extrinsic (resource exhaustion or process-level
  death on the runner), so it stays unchanged with a discriminator table in the flake log — capture the full panic text
  on the next occurrence. Both suites now carry concurrency caps (e2e `SLOTS`, the supervisor unit binary's real-tmux
  semaphore); lower them further only on evidence. Remaining parked test debt from the M5 and flake-fix review rounds —
  assertions that need seams the code deliberately does not have yet: a deterministic attach-races-rename interleaving;
  a takeover-before-marker forcing (needs a held-replay seam); a rename-commit cancellation test (needs a seam between
  the durable write and the map install); a claim-released-before-reply-probe test (needs a pausable tmux probe);
  proving the supervisor — not just the client — is mid-replay in the marker boundary test; forcing an
  outcome-transition failure during a rename's reply build (the capture-store fault seam covers capture writes only, and
  an unwritable database fails the title write first, a different path); a deterministic ack-enqueued observation for
  the upload-priority test (today a settle window with documented margins); and a component test of the scope probe's
  timeout-versus-retry path (needs injectable tool paths or timeouts — the probe deliberately has no fake-tools seam, so
  today the retry decision is pinned as a pure function only). The diagnosis instrument that series left behind then
  FIRED — pause-spam-detach, during M6's stack CI (2026-08-04), its first failure since it was individually closed — and
  what it captured is the next data point for the open server-death-under-load hypothesis: "reattach: tmux control
  client exited before the session-sink attach reply". A control client that announces its own exit mid-reattach, in a
  test whose own fix had nothing to do with the tmux server's liveness, is the same shape the pane-silenced case was
  hardened for, which makes this evidence about the SERVER rather than about either test. Stack CI is itself part of the
  picture: with a seven-PR stack, a push triggers overlapping full runs on one runner and the whole load-sensitive class
  fires more often, so the load the class is sensitive to is partly self-inflicted. Two candidates are recorded and
  neither is decided — a GitHub concurrency group per ref (cancelling superseded runs of the same branch), and lowering
  e2e `SLOTS` under CI — because both trade coverage or latency for quiet, and that trade wants evidence rather than a
  reflex. Separately on watch, not yet a class: two one-off SIGSEGVs in the supervisor lib test binary under
  full-workspace parallel load (2026-08-04 and 2026-08-05), both clean on rerun, no shared test and no pattern between
  them. A THIRD occurrence is a real bug to investigate — a segfault is not a timing flake, and the standing rule is
  that the next one is chased rather than quietly rerun. One product bug is parked here too, found through the e2e
  snapshot paths rather than by a test: an invocation of 120-odd characters overflows the session view's titlebar and
  pushes the rename control out of reach, which is a real UI failure on a surface every session has and not test debt at
  all.

  Amendment (2026-08-08, the M7 run): three of this entry's open threads closed. The SIGSEGV watch item concluded
  exactly as the standing rule demanded — occurrences reached seven, a core_pattern trap caught a core, and the
  backtrace named the mechanism: a failed `CommandExt::exec` leaves libc's `environ` pointing at the Command's freed
  envp in whatever process ran it, so the launch exec-failure tests were poisoning the whole test binary and any later
  `getenv` (glibc tcache safe-linking values are why every fault address looked like a truncated pointer). Test-only;
  production shims exec-or-exit. Fixed in the flake-hardening PR by moving those scenarios into child processes. The
  server-death-under-load hypothesis is now CONFIRMED — the pane-silenced diagnostics caught "%exit server exited
  unexpectedly" on a SERIAL stack-CI rerun — and of the two recorded CI-quieting candidates the evidence picked neither:
  serial reruns still failing rules out cross-run overlap, so the pressure is within-run thread oversubscription, and
  the remedy is `--test-threads=4` in the CI test job (matching the runner's vCPUs), landed at the stack bottom.
  Ack-ahead-of-backlog and the heartbeat-idle spec each recurred and were re-fixed at the evidence layer rather than the
  claim (prove the flood flows before soaking; extend the traffic window by elapsed time, not echo count). New parked
  observation, systemic rather than incidental: e2e harness state tempdirs leak on abnormal test-stack exit — 423 of
  them (~22 GB) accumulated across one long run before an ENOSPC — cleanup-on-abnormal-exit wants a design pass. Also
  folded in from the flake-fix series' staged notes (2026-08-03, recorded in its own log but never pushed here): one
  browser-suite sighting of the Playwright flood harness failing with "drain socket closed before FLOOD-DONE" — passed
  its rerun, one occurrence, the first candidate for a browser-side load class; a second occurrence earns it a
  discriminator entry of its own. That trigger fired during the M7 mopup run (2026-08-08): the full-suite WebKit pass
  timed out waiting 45 seconds for the whole-stream test's `FLOOD-DONE` marker while the in-page verifier was still
  advancing (701,657 of 800,000 records), and the focused rerun passed. This is a fixed-throughput-budget flake, not
  evidence of loss or a recurrence of the first sighting's premature raw-socket close. The test now waits on the
  verifier's constant-size progress state: corruption fails immediately, forward progress renews a bounded stall budget,
  and an independent hard cap still catches a producer that is only limping rather than stuck.
- **M6.75 — status and profiles.** Running/waiting/idle heuristics with per-agent sharpening, list filtering, profile
  CRUD and starter profiles. Also live push replacing BOTH of the UI's polls — M2's session-list poll and M4's
  session-detail/tab-list poll — status transitions are what make polling genuinely painful, and the push channel serves
  all of it. Moved from the old M5 slot to after multi-host (2026-08-03) deliberately, and not only for daily-driver
  sequencing: built here, the push channel is designed ONCE against its real shape — multi-host aggregation, the
  stale-view transitions the helm synthesizes from host connectivity plus helm.db's last-known cache, and cursor
  pagination all exist by now — where the old order built it single-host in M5 and reworked it in M6. Polling stays
  adequate in the gap precisely because the statuses that make it painful do not exist yet.
- **M7 — the outer ring.** Web-token auth and device sessions, `farhelm spawn` and agent-spawned sessions (deliberately
  late as well), archive, provisioning, Mac app bundling. Two items absorbed from M4's manual desktop pass (2026-08-03):
  an image FILE copied in Finder and pasted on macOS/WKWebView publishes under a generated `pasted-<n>.png` name instead
  of its own — the documented cost of `classify`'s origin heuristic, observed for real; revisit the heuristic (or
  wry-native clipboard hooks) here, and capture the WKWebView clipboard facts (File name, `lastModified`, item order) as
  part of that work. And the remote-screenshot paste-latency measurement (PLAN_M4.md acceptance 9's `--ssh` step) was
  deferred to daily use — record a number here if dogfooding has not already surfaced one. Planned in detail in
  PLAN_M7.md.
