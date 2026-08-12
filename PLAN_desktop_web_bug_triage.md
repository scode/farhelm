# PLAN: desktop/web bug-triage observability

A goal file for an agent session (`/goal PLAN_desktop_web_bug_triage.md`). It describes a small, bounded feature: make
the desktop app self-reporting enough that a webview-side failure — especially the MT-5 class, where the eval bridge
dies and the UI silently bricks — produces greppable evidence in the log the dev flow already collects, and make CI
assert that the reporting pipeline itself works. This exists because the desktop webview is the one layer of the stack
that fails invisibly: native Rust has tracing, browsers have devtools, but a wry webview error today goes nowhere.

NOTE: macOS CI for ordinary portability is a separate effort and out of scope here. So is anything that changes the
user's dev loop: the deliverable must add zero new steps, artifacts, or paste targets to bug reporting. The whole point
is that `desktop.log` (already captured by scripts/laptop-dev.sh) simply starts containing more of the truth.

## Decisions already made (do not re-ask)

- **Merge policy: leave the stack open.** Create the PRs, get each swarm-reviewed and battery-green, and stop. The user
  lands the stack (and runs the e2e suite at that point, per CLAUDE.md's stack-merge gate).
- **Bridge death response: log only.** No auto-reload, no process exit. The user notices a bricked UI on their own; the
  log's job is making the subsequent report instant.
- **Capture scope: desktop only.** The shim activates only under the desktop bootstrap. Browsers have devtools; do not
  forward their consoles. The helm endpoint exists unconditionally but nothing sends to it from a browser.
- **Transport: loopback HTTP to the embedded helm, never the eval IPC channel.** The bridge dying is the marquee
  failure; evidence must not travel over the thing being monitored.
- **Log destination: native `tracing`.** Forwarded webview events become ordinary tracing events (target
  `webview_console`, level mapped from the console method). They land wherever native logs land — which in the dev flow
  is `desktop.log`. No new files, no ring buffers, no rotation machinery.
- **Scrubbing: forward console/error message strings plus a small capped source label, nothing else.** Never DOM
  contents, never IPC payloads, never terminal bytes, never anything that could carry the token (the same posture the
  desktop bootstrap already keeps for its own credential: never in a URL, page markup, or a command line — logs get the
  same respect). Both forwarded strings are bounded and control-escaped server-side (message 2 KiB, source 256 B, via
  the helm's existing peer-text convention), and the stream is capped at 60 accepted entries per fixed 60-second window
  (a boundary burst of up to 2× is the accepted cost of the fixed window), with at most ONE dropped-count line per
  window so the drop report can never itself become the flood.

## Design

Three small mechanisms and one document:

1. **Helm endpoint** (`crates/farhelm-helm`): `POST /api/client-log`, authenticated exactly like every other device-
   session route. Body: exactly `{"entries": [{level, message, source?}, ...]}` — an envelope object, not a bare array,
   so unknown top-level fields are refused today and the envelope can grow later without breaking the array shape.
   Server-side truncation, escaping, a route body limit, and rate-capping as a second line of defense behind the shim's
   own caps. Handler emits each entry as a tracing event and returns 204 for every authenticated, parsed request.
   Bounded and boring; reuse the existing auth extractor and peer-text conventions.

2. **Webview shim** (`crates/farhelm-ui` asset, loaded first among the page scripts): wraps `console.error` and
   `console.warn`, hooks `window.onerror` and `unhandledrejection`, queues entries, and flushes batches to the endpoint.
   Desktop-only activation: the shim arms itself when the desktop bootstrap hands it the API base and credentials —
   which resolves the chicken-and-egg with auth by **buffering until the desktop device session exists, then flushing**
   (bounded buffer, oldest-dropped). Errors thrown before auth completes are exactly MT-5 territory; the JS side may
   lose them if the bridge dies first, and that is accepted because the native watchdog below covers that window.
   Console wrapping must be transparent (call through to the original, never swallow, never re-enter itself on its own
   failures — a shim error must not recurse).

3. **Native bridge watchdog** (`crates/farhelm-ui` desktop side): a periodic eval-channel heartbeat (generous interval,
   ~15s; two consecutive misses = dead) that, on failure, emits one loud tracing error naming the condition ("webview
   eval bridge is not answering; the UI may be bricked (MT-5 class)") and then keeps checking quietly in case it comes
   back. Log-only per the decision above. The heartbeat must be cheap and must not fight Dioxus's own eval lifecycle —
   if a persistent eval loop proves fragile, a per-tick one-shot eval is fine; pick whichever survives a review of
   dioxus-desktop 0.7's eval semantics, and document the choice.

4. **Triage recipe** (`docs/desktop-web-triage.md`, linked from CLAUDE.md in one line): the engine-discrimination
   decision tree (broken in app but fine in Safari → wry/bridge layer; broken in Safari and app but fine in Chromium →
   WebKit divergence in the JS island; broken everywhere → shared logic), where the log lives in the dev flow, what a
   bridge-death entry looks like, and what to include when handing a bug to an agent (the log path — nothing else).

CI: extend `scripts/desktop-smoke.sh` with one assertion leg — under a smoke-only env var (follow the existing
`FARHELM_SMOKE_*` convention), the shim emits a marker error at startup, and the script asserts that marker appears in
the captured log. That single check proves shim → endpoint → tracing end-to-end, which is the pipeline whose silent
death would otherwise only be discovered during an incident. Also assert the watchdog's heartbeat has NOT fired its
death message during a healthy smoke run (guards against false positives). Keep it to those two greps; the smoke
script's fragility budget is spent.

## PR sequence (linear jjstack stack, bottom first)

1. `pr/triage-endpoint` — helm `POST /api/client-log` + tests (auth required, caps enforced, entries land as tracing
   events; reuse the crate's existing router test harness).
2. `pr/triage-shim-watchdog` — the shim asset, desktop-only arming with buffer-until-auth, the native watchdog, and
   their tests (JS-side pure functions get js-tests coverage per the existing `crates/farhelm-ui/js-tests` harness;
   watchdog logic behind a seam if timing needs injecting — follow the repo's existing seam conventions, and do not
   over-build: one seam at most).
3. `pr/triage-smoke-and-docs` — the desktop-smoke assertion leg, `docs/desktop-web-triage.md`, and the CLAUDE.md pointer
   line.

Each PR: full CI battery from CLAUDE.md's "Finishing work" list before creation; commit messages and PR text through the
scode-commit-msg-reviewer gate; `pre-pr-review-swarm` run against the PR's diff before it is created, with reviewer
subagents routed to **gpt-5.6-sol** (galaxy-brain owns the routing; sol high for correctness/security reviewers, sol
medium acceptable for prose/style lenses). Review findings get fixed or explicitly rejected with a reason in the log.
The stack stays open at the end; do not merge.

## Process contract

- **Delegation:** scode-galaxy-brain is active for the whole goal. Orchestration, design judgment, VCS, and final
  quality gates stay in the driving session; implementation units route per the work-profile table; announce routing
  choices as the skill requires. If codex rate limits are hit, pause and say so — never work around them.
- **VCS:** jjstack, one reviewable commit per bookmark per PR, linear stack based on `main` (or on the still-open
  `pr/macos-boot-id` only if main has not absorbed it and conflicts force an ordering — prefer `main`).
- **Progress log:** `../log-desktop-web-bug-triage.md` (sibling of the repo root, deliberately outside the repo). This
  file IS the resumable state: any future session must be able to start from `/goal ../log-desktop-web-bug-triage.md`
  alone. First action of the first session: create it. Update it after every state change (a commit made, a PR opened, a
  review round finished, a decision taken, a blocker hit) — write as if the session dies on the next tool call.
  Contents, always current: a pointer back to this plan; the decisions table above (so drift is detectable); a stack
  table (bookmark → commit → PR number → status: building / swarm-review / battery / open / blocked); the last action
  completed; the exact next action; and a gotchas section for anything learned the hard way (failing commands, flaky
  tests, review findings rejected and why). On resume: read the log, read this plan, reconcile against `jj log` and
  `gh pr list` (the log records intent; the repo records truth), then continue from "next action".
- **Unattended-execution rule:** the decisions above plus this plan should cover the foreseeable choices. If something
  genuinely outside them comes up, prefer the smallest documented fallback that keeps the goal's non-goals intact (no
  new user-facing steps, no eval-IPC transport, log-only responses), record the choice prominently in the progress log,
  and continue. Stall only if the fallback would violate a decision the user made explicitly.

## Acceptance

- A JS `console.error` or uncaught exception in the desktop webview appears in the native log within seconds, scrubbed
  and truncated per the rules above; nothing is forwarded when the same UI runs in a browser.
- Killing the eval bridge (or any condition where it stops answering) produces the single loud watchdog line; a healthy
  run produces none.
- `scripts/desktop-smoke.sh` fails if the shim→endpoint→tracing pipeline breaks, and fails if the watchdog
  false-positives during a healthy run.
- `cargo test`, js-tests, clippy, fmt, dprint, desktop check, tmux suites: green per the standard battery.
- The user's flow is unchanged: same laptop-dev.sh, same `desktop.log`, one doc to point agents at.

## Known risks (where this could get bigger than it looks)

- **Auth ordering.** The buffer-until-auth design is the accepted answer; if the desktop auth flow turns out to make
  even that awkward (e.g. no clean signal for "device session ready" reaches the shim), do NOT invent an unauthenticated
  endpoint — that trades a log feature for a hole in the security posture. Fall back to arming the shim post-auth only
  and record the narrowed coverage in the triage doc.
- **Watchdog false positives.** A busy webview (giant terminal writes) may answer heartbeats slowly. The two-miss rule
  and generous interval are the guard; if smoke or daily use shows flapping, widen before adding cleverness.
- **Smoke fragility.** The assertion leg must stay two greps. If it needs more than that to be reliable, cut it back to
  the shim-marker grep alone rather than growing the harness.
- **Dioxus eval semantics.** The watchdog's eval usage must be checked against dioxus-desktop 0.7.10's actual eval
  lifecycle (channel reuse, drop behavior) before implementation, not assumed — MT-5 exists because assumptions here
  were wrong once already.
