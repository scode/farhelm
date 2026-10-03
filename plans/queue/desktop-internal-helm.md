# Desktop internal helm: the desktop app's helm serves no browsers and listens only for its own window

Written against main at dc1ec664 on 2026-10-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a passage the user decided to change.

This plan has no dependency on another plan. It builds directly on #1487, from the landed `triage-signin-recovery` plan,
which already mints the desktop app's two credentials in memory at launch. Read that PR's diff before starting.

## The goal

The desktop app embeds a helm and shows the web UI in a webview. Today the webview and the app's native side talk to
that helm over HTTP and WebSockets on a fixed loopback port (7433), exactly as a browser does, and the helm also serves
its UI and its token sign-in to any browser on the machine. The user decided that, in the desktop app, the helm and the
web UI are internal implementation details. After this plan:

1. The desktop app's helm serves no browser UI, offers no token exchange, and accepts only the two per-launch
   credentials it mints in memory at startup (#1487's embedded credentials). Stored device credentials, including old
   desktop credentials persisted before #1487, no longer authenticate against it.
2. It listens on a random loopback port chosen by the kernel at each launch, not 7433, and nothing outside the app needs
   to know which.
3. The `dioxus://`/`wry://` origin exemption and the desktop webview's cross-site (CORS) answers exist only on the
   desktop app's helm. A standalone helm (`farhelm helm run`) has neither.
4. The webview keeps its credential in page memory, not in webview storage (WebKit writes `localStorage` to disk).
5. SPEC.md, SPEC_impl.md and the docs say so; the desktop smoke test still covers what it covers today, through a
   debug-build-only seam instead of the port.

The terminal and event-feed WebSockets keep working exactly as they do: same sockets, same flow control, same helm
handlers. Nothing moves onto the webview's eval bridge.

Acceptance criteria, overall: the desktop app works as before (sessions, terminals, attachments, clipboard, diagnostic
log, takeover between its own window and nothing else); a browser pointed at the desktop helm's port gets no UI and
cannot obtain or use a credential; a standalone helm behaves exactly as today except that a `dioxus://` or `wry://`
Origin is refused; the desktop smoke test passes; specs and docs match.

## Requirement sources

**The user's request:** "using planning system to plan "desktop without a network port" and ..." (the other entry is a
separate plan). The TODO.md entry, verbatim as of dc1ec664:

- "**No network path for the desktop app.** The desktop app's webview talks to its embedded helm the way the browser
  does, over HTTP and WebSockets on a loopback port, so one UI code path serves both clients. That port can be reached
  by every process on the machine, including other accounts' (the credential stops them from using it), and the helm's
  browser defenses need a scheme-level exemption for the webview's `dioxus://` origin, which every Dioxus desktop app
  shares. Consider moving the desktop client to an in-process transport instead, as Tauri commands or Electron IPC do:
  with nothing listening, nothing else on the machine can reach the desktop's API, and neither the port nor the origin
  exemption is needed. The hard part is streaming: wry's custom-scheme handler answers each request with one complete
  response, so terminal output and the event feed would have to travel over the webview's IPC channel, and the UI's
  network layer would need a second transport beside HTTP."

**The user's decisions (2026-10-03):**

- U1. "When you run the desktop app, the helm and the web stuff is internal implementation detail. This is a change from
  my early desires that led to the spec, because I want to constrain complexity." SPEC.md changes accordingly (see Spec
  changes below): the desktop app's helm is the exception to "the helm always serves a browser UI".
- U2. Design (A), "framework-style", chosen over a port-free design: keep a loopback listener, as Dioxus itself does for
  its own UI updates (dioxus-desktop's `edits.rs` runs a WebSocket on a random loopback port guarded by a random key,
  because its custom-protocol path had bugs), but on a random port and usable only with credentials minted in memory per
  launch. Rejected alternative, so you do not drift back into it: carrying terminals, the event feed and uploads over
  the eval bridge (`evaluate_script` per chunk, base64, hand-built flow control), which would invent a transport on a
  channel that has frozen on macOS before and would move the app's most important traffic onto it.
- U3. Dioxus's own loopback port stays. It is the framework's, and removing it would mean maintaining a fork. Document
  it as accepted.
- U4. The origin exemption and the CORS answers stay for the desktop's own helm, because the window's page is
  `dioxus://index.html` and the helm is `http://127.0.0.1:<port>`, which are cross-origin whatever the port, but they
  are removed from the standalone helm, which never has a desktop window as a client. Routing the page's one-off
  requests through a custom-scheme handler instead was rejected: WebSockets cannot go that way, and uploads would be
  buffered whole, breaking SPEC_impl.md's "a large file costs time rather than memory".
- U5. Token commands are untouched. The desktop app and a standalone helm share the default state directory, so the web
  token and `farhelm helm token show/rotate` keep working for a standalone helm started on that directory later. Against
  a running desktop app they simply have no effect, because its helm no longer accepts token-derived credentials.
- U6. Browser-based triage of the desktop app is replaced by quitting the app and running `farhelm helm run` on the same
  state directory. The user does not care about the maintainer-local `laptop-dev.sh` dev flow; do not preserve or
  investigate it.
- U7. Review gate: a fresh-context Opus 5.5 reviewer at high effort, general charter, no swarm.
- U8. No-workhorse mode: you do all the work yourself (see How to run).

**Planner choices, shown to the user without objection:**

- P1. The desktop smoke test gets a debug-build-only seam that tells it the port and gives it a test credential, gated
  at compile time, never by an environment variable alone in a release build (see PR 1).
- P2. `FARHELM_DESKTOP_PORT` and `FARHELM_DESKTOP_UI_DIST` are removed. The second only ever changed what the helm
  served over HTTP to browsers, never the window's bytes (`crates/farhelm-desktop/README.md`, SPEC_impl.md Native app
  packaging).
- P3. The window's failure page and its retry stay. #1487 kept them deliberately: a broken start (an IPC failure, a
  validation timeout, a failed socket probe) can still reach them with per-launch credentials, and SPEC.md Signing in
  again requires the retry. This plan does not remove any of the desktop's credential machinery that #1487 left; it only
  stops the page persisting its credential (PR 3).

**Binding repository constraints:**

- SPEC.md Security, "Client to helm": "Nothing that establishes the native app as a client may rely on `Origin`; that
  rests on the credential the native process obtains itself." Keep that: the per-launch secrets are the gate, the
  exemption is not.
- SPEC.md Security keeps the helm's `Host` check (DNS rebinding) on every helm, the desktop's included.
- SPEC_impl.md, the attachment upload path streams under a credit window; do not buffer uploads.
- `lore/PLAN_desktop_web_bug_triage.md`: the diagnostic client log goes over loopback HTTP, never the eval bridge, so
  diagnostics survive a bridge failure. That stays true.
- `claim_serving_ownership`'s state-directory lock remains what stops a second helm on the same directory. With a random
  port it is the only refusal; a second desktop instance must still be refused visibly (the desktop's existing
  refuse-and-exit path).
- Root `AGENTS.md`: Harness-specific code does not apply; Sharing the machine with other agents (no fixed ports in
  harnesses) applies to the smoke rewrite; a `feat`/`fix` PR carries a changelog fragment.

## Background, verified on main at dc1ec664

Find code by name; line numbers drift.

- Startup: `DesktopBootstrap::start` in `crates/farhelm-ui/src/desktop.rs` reads `FARHELM_DESKTOP_PORT` (default
  `DEFAULT_DESKTOP_PORT` = 7433) and `bundled_web_ui()` (`FARHELM_DESKTOP_UI_DIST`), then calls
  `farhelm_helm::run_embedded(HelmArgs { .. }, ..)` on its own thread. `run_with_ready` in
  `crates/farhelm-helm/src/lib.rs` binds `127.0.0.1:port` (port 0 already works; it reads `local_addr`), claims the
  state directory, opens the store, serves token control, mints the web token, picks a UI source (`select_ui_source`
  with `embedded_ui()`, compiled into release builds, so today the desktop helm serves the UI over HTTP even though the
  window never loads it from there), builds the router, prints `farhelm helm: http://{addr}/`, and on the embedded path
  sends `EmbeddedReady { addr, native_device_secret, webview_device_secret }`.
- Credentials: `AuthState::mint_embedded_device` in `crates/farhelm-helm/src/auth.rs` keeps digests in memory;
  `accepts_device` checks those first (`AcceptedDevice::Embedded`, unrevocable sockets) and then stored rows
  (`AcceptedDevice::Stored`).
- The window's assets come from `use_embedded_asset_handler` (`crates/farhelm-ui/src/desktop/assets.rs`), reading
  `embedded_ui()` directly, not from the helm's HTTP side.
- Native REST (`crates/farhelm-ui/src/api.rs`, reqwest) sends no `Origin` and passes the guard on `Host`.
- Page JS that calls the helm directly: `terminal.js` (terminal WebSocket; attachment upload `fetch` with `Bearer`),
  `events.js` (`/api/events` WebSocket), `client-log-shim.js` (`POST /api/client-log`), the clipboard writer armed by
  `arm_native_clipboard_script` (`POST /api/clipboard`), `desktop-auth.js` (`GET /api/auth/device` and a probe
  WebSocket). `terminal.js` and `events.js` read the secret from `localStorage` (`farhelm.device-secret`), which
  `desktop-auth.js` writes; the shim and the clipboard writer get it from eval-armed globals.
- Origin guard and CORS: `crates/farhelm-helm/src/middleware.rs` (`require_loopback_origin`, `origin_is_allowed`,
  `is_desktop_webview_origin`, `desktop_webview_cors`, `desktop_webview_preflight`), wired in `build_router` and
  `api_router`, on `/api/auth/device`, `/api/auth/token`, attachments, `/api/client-log` and `/api/clipboard`.
- `scripts/desktop-smoke.sh` picks a free port, passes `FARHELM_DESKTOP_PORT` and `FARHELM_DESKTOP_UI_DIST`, waits for
  the HTML at `curl $API/`, mints a browser credential through `farhelm helm token show` and `/api/auth/token`, drives
  hosts, sessions and preferences with curl, reads `webview_auth_generation` from `desktop-client.json`, asserts device
  row counts, and has a token-rotation leg. It builds a debug desktop binary, so `cfg(debug_assertions)` code is present
  in it. Existing runtime smoke hooks: `FARHELM_SMOKE_CLIENT_LOG_MARKER`, `FARHELM_SMOKE_TMUX_MARKER`.

## Implementation outline

Three PRs, in this order.

### PR 1: the desktop app's helm serves no browsers and binds a random port (`feat!:`)

- An embedded mode in the helm, chosen by the embedded entry point (not by an environment variable): no UI source (force
  the equivalent of `UiSource::None` on the embedded path, and suppress the "no UI" warning that would otherwise log on
  every desktop launch; `embedded_ui()` itself stays compiled in for the window's asset handler), no `/api/auth/token`
  route, and `accepts_device` accepts only embedded credentials. Keep it one flag or one small enum threaded from
  `run_embedded`, not a second router. Leave the legacy `localhost` redirect alone: the window never uses those hosts,
  and it is the only reason this PR would touch the middleware.
- Token control and the web token stay as they are (U5).
- `DesktopBootstrap::start` binds port 0. Remove `FARHELM_DESKTOP_PORT`, `DEFAULT_DESKTOP_PORT` and
  `FARHELM_DESKTOP_UI_DIST`, with their docs (`crates/farhelm-desktop/README.md`, the comment that "a stable origin
  makes the embedded web UI discoverable", the `run_embedded` and `HelmArgs.port` docs, `embedded_ui.rs` docs). The
  build-time `FARHELM_UI_DIST` is a different variable; leave it. Drop or reword the `farhelm helm: http://…` stdout
  line on the embedded path so it does not invite opening a browser.
- Second instance: confirm a second desktop launch on the same state directory is still refused visibly now that the
  bind no longer collides (the ownership claim waits up to `SERVING_CLAIM_WAIT`, then refuses); adjust the message only
  if it no longer makes sense for the desktop.
- Smoke seam (P1): in debug builds only, when the smoke asks for it, the desktop writes its address and its existing
  native credential to a 0600 file the smoke names, and the smoke drives the API with that credential instead of the
  token exchange. Do not mint a third credential: that would need new helm API behind a debug gate in a second crate,
  and would give `mint_embedded_device` a second caller its docs rule out. The port and credential change on every
  launch, so every leg that relaunches the app (the restart leg, the answering-supervisor leg) re-reads the file after
  each launch. Keep `webview_auth_generation` and the other readiness evidence as they are. Remove the rotation leg's
  assertions that only made sense while the desktop accepted token-derived credentials, and add an assertion that the
  desktop helm's port serves no UI and refuses `/api/auth/token`. Keep the pre-display process legs as they are.
- Spec and docs (see Spec changes): Topology, Acceptance test steps 1 and 6, Security "Client to helm" (the desktop's
  credentials, no exchange), Upgrade compatibility and client scale (the exempt-credential sentence), SPEC_impl.md Helm
  internals and Native app packaging, `docs/security.md`, `docs/manual-mac-checklist.md` steps 2 and 7,
  `docs/desktop-web-triage.md` (U6: the engine-discrimination tree now compares the app against a standalone
  `farhelm helm run` on the same state directory after quitting the app, not side by side).
- Tests: helm unit or REST-harness tests that the embedded mode serves no UI, has no token route, and refuses a stored
  device row; the smoke rewrite.
- Changelog fragment `kind: removed` (opening the desktop app's helm in a browser no longer works; run a standalone helm
  instead).

### PR 2: the origin exemption exists only on the desktop's helm (`fix:`)

- `origin_is_allowed` accepts `dioxus://`/`wry://` origins only in embedded mode. Gate only that check: once a
  standalone helm refuses those origins, its desktop CORS layers and preflight routes can never fire, so making them
  conditional too would add a second router shape with no observable effect. Keep the `Host` check in both modes.
- Tests: turn `custom_scheme_origins_are_allowed_but_null_is_not` and
  `cross_site_navigations_are_refused_without_a_vouching_origin` into a pair per mode: allowed in embedded mode, refused
  standalone.
- SPEC.md Security (the exemption paragraph: it now applies only to the desktop app's internal helm, and why it is still
  needed there: the window's page is `dioxus://index.html`, cross-origin to the helm), SPEC_impl.md loopback-guard
  sentence, `docs/security.md`, the `middleware.rs` module docs.
- Changelog fragment: choose `kind` per `releasing/EDITORIAL_GUIDANCE.md` (a standalone helm now refuses other
  framework-built apps' pages; `none` with a reason if you judge nothing user-facing changed).

### PR 3: the window keeps its credential in memory (`refactor:` or `fix:`)

- `terminal.js`'s `deviceSecret()` and `events.js`'s `deviceProtocols()` read a page global that the desktop sets over
  the existing hand-off (as the client-log shim and the clipboard writer already do), and fall back to `localStorage`
  when it is absent. Both files are shared by the browser and desktop builds, so "global if present, otherwise storage"
  is the split; the browser's behavior does not change. `desktop-auth.js` stops writing `farhelm.device-secret` and
  removes any value an earlier version left there.
- Nothing else in the desktop's credential machinery changes (P3).
- SPEC_impl.md wherever it describes the webview's storage of its secret.
- The type reflects the user-visible effect: this is hygiene, following #1487's "no secret on disk" direction, not a
  security fix. `refactor:` with no fragment unless you find a user-visible effect; then a modest `fix:` with one.
- This is the plan's last code PR: it removes the TODO.md entry "No network path for the desktop app".

### Spec changes (U1)

- SPEC.md Topology: replace "The native app's embedded helm serves the web UI too." The desktop app's helm is an
  internal part of the app: it serves only the app's own window, on a loopback port of its own choosing, and serves no
  browser. To use Farhelm from a browser, run a standalone helm. Adjust "These are the only two faces the helm has — a
  web UI, or the local app embedding it." to stay true.
- SPEC.md Acceptance test: step 1's "Also open the same helm's web UI from a browser (token-authenticated)" and step 6's
  "Attach to the remote session from the web UI; the native app visibly detaches" move to a standalone helm with the app
  quit. SPEC.md says exactly one helm runs at a time, so the steps must not have a standalone helm running beside the
  app; show the takeover between two browser tabs of the standalone helm instead. Keep the acceptance test meaningful;
  do not just delete the steps.
- SPEC.md Security: the desktop's helm accepts no token-derived credentials and serves no UI; note that Dioxus itself
  keeps a loopback WebSocket for its own UI updates, protected by a random per-launch key (U3).
- SPEC.md One GUI at a time ("one browser tab or the desktop app's window") stays true; check the surrounding text.

### What not to build

- No eval-bridge or custom-scheme transport for terminals, the event feed, uploads, clipboard or the client log.
- No Dioxus fork or patch.
- No change to the standalone helm's sign-in, token, UI serving or CORS beyond removing the desktop exemption (PR 2).
- No new environment variable that can loosen a release build.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-desktop-internal-helm-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/desktop-internal-helm/<nn>-<short-name>`.
- PR 1, PR 2, PR 3 in that order, one commit, bookmark and draft PR each. Within this run, if a PR needs correcting,
  restructure it rather than stacking a correction on top, and do not add code in one PR that a later PR of this plan
  deletes.
- Conventional Commits, types as suggested per PR above (the type reflects the user-visible effect; PR 1 carries `!`
  because it removes a documented capability). Each `feat`/`fix`/`!` PR adds its changelog fragment under
  `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression in each PR, through
`scripts/record-test-run.py` for test execution (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Typical choices, not a checklist:

- Every PR: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo check -p farhelm-desktop`, `dprint check` on changed Markdown.
- The helm's auth, middleware and REST-harness tests that cover the embedded mode, the origin guard and CORS; the
  desktop-feature nextest selection (`-p farhelm-ui --features desktop`) for the bootstrap and desktop modules.
- The desktop smoke (`scripts/desktop-smoke.sh` with the pinned tmux, per root `AGENTS.md`) for PR 1 and PR 3, since
  both change what the window or the smoke depends on. It needs Xvfb and the webkit2gtk dev packages; if this host
  cannot run it, say so in the report as a check skipped with its reason rather than claiming coverage.
- PR 3 changes `terminal.js` and `events.js`, which the browser build also runs: run the Playwright specs that cover
  terminal attach and the event feed on Chromium and WebKit through the recorder, building per root `AGENTS.md` first,
  to show the browser path is unchanged. Not the full suite unless a specific risk needs it.
- `python3 releasing/check-changelog.py format` for every fragment.
- Any PR that changes Rust or browser tests or their helpers: `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate one review of that PR's changes. The user
demands a fresh-context agent on Opus 5.5 at high effort, shelled out to the other harness if the executing one cannot
reach that model natively. No review swarm. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria for that PR: its section above, The goal, U1-U8 and P1-P3. For a
PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md`
requires. Ask the reviewer specifically whether anything still lets a browser obtain or use a credential on the
desktop's helm, and whether the standalone helm lost anything other than the desktop exemption. Address what the
reviewer finds before moving on, and log the DECISION where you decline a finding. Do not write a launch command here or
in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (any eval-bridge or custom-scheme transport, a second
router, a new credential type, a Dioxus patch, a new runtime switch, keeping the fixed port; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the request, the user decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular how embedded mode is threaded, what the embedded helm prints, the smoke seam's
shape and gating, the second-instance refusal, the rewritten acceptance-test steps, each changelog kind, and every
reviewer finding you declined. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. In
particular, if the desktop window turns out to need something from the helm's browser-facing side that this plan
removes, or the smoke cannot keep its current coverage without a release-build switch, block rather than reintroduce it.
Anything that needs a decision: record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step
10). Because the PRs form one linear stack, if one PR blocks, do not build the later ones on top of it.

## Done criterion

The plan is complete when the three draft PRs exist as one linear stack, each satisfies its section above and the
acceptance criteria, each has passed the review gate, and the last has removed the TODO.md entry. Open, not merged:
merging happens only after the maintainer has reviewed this plan's report. If a `## Decisions` section exists, its
latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.
