# TODO

A running list of things the maintainer wants fixed or built. This is intent, not history: an entry is REMOVED in the
same PR that addresses it, so the file only ever describes what is still wanted. It is not a roadmap and carries no
priorities unless an entry says so itself.

Thirteen buckets, assigned by the maintainer: "definite simplification" is complexity the maintainer has decided to
remove — the decision is made, only the work remains; "planned" holds accepted work to implement later and suppresses
duplicate review triage within each item's stated scope; "plan decisions to revisit" holds the calls made while a plan
ran or landed, often on the maintainer's behalf when a plan blocked, each recording the alternative; "near term" is what
should be picked up next; "linux support" holds Linux desktop app installation, packaging, and Linux-specific feature
work; "mid term" is wanted after the near-term work, sooner than "maybe later"; "doc todo" holds documentation work;
"tricky bugs" retains unresolved bug reports and their investigation findings; "deflake" gathers test and harness
reliability work, including CI execution and restoring gates; "broken tests" records tests that fail deterministically,
with the failure and the evidence that it predates any in-flight work; "code cleanup" holds structural code smells from
a whole-codebase assessment, grouped by kind with a suggested order; "maybe later" is wanted but not soon, and may never
happen; "unbucketized" is everything not yet sorted, which carries no implication either way. Within a bucket, no order
unless the bucket explicitly says so.

Near term's "Plan follow-up" subsection holds the entries that came out of plans (follow-ups asked for in review,
leftovers from reports and landing notes), kept apart from the rest of the near-term work.

Known product fixes stay in their product bucket. "Difficult deflake" retains unresolved failures and their
investigation evidence within "Deflake"; that placement does not establish that the cause is test-only. Move a diagnosed
product fix out of "Deflake" rather than changing user-visible behavior as a test correction.

## Definite simplification

## Planned

## Plan decisions to revisit

- **Revisit Clone and Replace with starting old no-permission sessions as YOLO.** On 2026-11-23 the landing monitor,
  landing `remove-own-setting-permission` (#4669, #4672, #4673, fix #4685), accepted a consequence of migrating old
  agent sessions that never chose a permission to YOLO: Clone, Replace with and Restart with of such a session now
  preselect YOLO and pass the agent's bypass flags, where before they composed a Guarded launch for agents that offer
  it, and a Clone of one makes YOLO the remembered default. Hosts that ask before YOLO launches still ask and offer to
  start guarded instead; hosts set to start YOLO without asking start it silently. The release-notes draft says so. The
  alternative was to leave those sessions' stored permission empty and rely on the rule that already counts an empty
  permission as YOLO for confirmation, which keeps Clone and Replace with composing Guarded but departs from the
  decision to migrate them to YOLO. Plan: `plans/queue/remove-own-setting-permission.md`.

- **Revisit giving each repository cache its own directory path.** On 2026-11-23 the landing monitor answered
  `repo-cache-lock-redesign`'s block on the maintainer's behalf: recovery from a stuck repository cache gives each new
  cache its own directory, with a small Farhelm-owned record selecting the current one, plus a migration of existing
  caches and cleanup of an interrupted rebuild. The executor found that a Git process surviving an interrupted cleanup
  can reopen the cache's original absolute path, so recovering in place could let it touch the replacement. The
  alternative was to keep the current layout and defer checkout recovery, leaving sessions on an affected cache able to
  fail at checkout. Plan: `plans/queue/repo-cache-lock-redesign.md`.

## Near term

- **A slow host still delays ordered desktop actions.** The desktop app keeps live updates flowing during host waits.
  Session Stop and Delete, environment Suspend, Resume, Stop and suspend, Delete and Retry-delete, and Checkout Trash
  listing and deletion now run independently of ordered commands. Credential replacement is the next candidate. It waits
  at most 20 seconds total for delivery across hosts, with up to four deliveries in parallel. The remaining ordered work
  includes launcher reads and Launch, session Rename, Restart, Replace, tabs and tunnels, and host management and
  settings, including Delete sandbox or container from the Hosts panel. Review the two-lane channel and the operation
  boundaries before converting more of them. Plan: `plans/queue/slow-host-async-actions.md`.

- **Idle CPU of the GPUI desktop app.** Accepted for now on 2026-11-16: an idle window with one running session uses
  about 3.05-3.09% CPU on the test Mac (Apple silicon), against 1.61-1.62% for the last browser-UI build in the same
  session. Frames are back to the old app's rate (3.0 changed frames a second, after the cursor blink was aligned with
  the running pulse); the whole gap is cost per frame, about 4.4 ms against 1.8 ms, spread over the session list (+0.86
  ms), the session header and tabs (+0.76 ms), whole-tree layout (+0.74 ms) and the shared buttons. Most of it is GPUI's
  own per-element work. Levers looked at and not taken: lighter Button and IconButton (about 1% of a frame), repainting
  only the pulsing dot (a vendored GPUI patch; helps only pulse frames), and caching the session list and header behind
  the cache wrapper, the one large lever, which drops a cached view's screen-reader information unless GPUI is patched
  to replay it. Look into caching with that patch, or another way to cut per-element cost.

- **Recognize Muse Code questions and approvals.** Detect questions and approval dialogs during a conversation from real
  Muse screens. These can currently look idle while Muse waits for an answer. Capturing them needs an authenticated test
  provider or a documented provider fixture protocol that can ask a real question. Plan:
  `plans/queue/muse-session-tracking.md`.

- **Run the desktop app's own supervisor as the same launchd job remote Macs get.** Asked for by the maintainer on
  2026-11-08 while planning remote macOS hosts: once that plan has landed and remote Macs are validated as stable, move
  the local Mac's supervisor from being started detached by the desktop app to the same per-user launchd job, so every
  Mac uses one consistent mechanism. Related: "Let the Mac's supervisor outlive the desktop app" under Maybe later.

- **Decide how terminal snapshots are supposed to work.** Every checkpoint of a terminal in a terminal host encodes the
  whole terminal (screen, up to 12,000 lines of scrollback, modes) into one self-contained blob, even though most of the
  scrollback has not changed since the previous snapshot. That repeated work is what makes typing pause during a
  checkpoint and what makes the burst at an orderly supervisor stop expensive. The plans `faster-snapshot-encoder` and
  `stop-checkpoint-burst` only make today's whole-terminal encode cheaper and rarer. The maintainer takes a position
  first, in particular on whether snapshots should be maintained incrementally, and implementation follows. Two shapes
  came up while planning those plans: incremental on the host (a base snapshot plus a chain of changes, which needs a
  new snapshot format and a protocol change for hosts that outlive every release), or incremental in the supervisor
  (cache the encoded bytes of history rows and re-encode only the screen and new rows, producing the same v1 blob as
  today). The repository records no reason for the original choice of whole snapshots.

- **Credentials for ordinary SSH hosts.** The Credentials section of Settings serves environments through their recipes.
  Extend it so ordinary SSH hosts can use the same named credentials.

- **Cut the new desktop app's idle CPU with a large running fleet.** With the session list now drawing only the rows on
  screen, idle CPU no longer depends on how many sessions are listed, and typing stays at about 30 ms at any fleet size.
  But it still grows with how many sessions are running: on the test Mac, 1.6% with 5 sessions, 3.3-3.5% with 179 of 200
  running and 5.3-5.4% with 449 of 500 running, against the old app's 1.6% idle. Most of it remains when the window is
  in the background, so it comes from applying the helm's updates for running sessions, not from drawing. Asked for on
  2026-11-14; first measure the old app with 200 running sessions for a fair comparison, then profile what each update
  costs in the app.

- **Remove the "Remove the bundled tmux" step from Add and Update.** Since tmux-drop-from-install (#3371 to #3374),
  every Add and Update plan for a remote Linux host includes a step that deletes the tmux copy earlier releases
  installed beside Farhelm. It is needed while Update can reach a host still carrying that copy: any release from 0.23.0
  (the current upgrade floor) up to the one before tmux was dropped may have installed it. Remove the step, its
  explanation, docs and tests once the upgrade floor passes the release that dropped tmux, or together with "phase 3"
  (Mid term), whichever comes first; a host updated after the step is gone keeps an unused copy, which is harmless.
  Decide the timing when this is picked up. Recorded by the maintainer on 2026-11-18 from the plan's review.

- **Finish the tmux cleanup: the known leftovers and one final sweep.** tmux-drop-from-docs (#3381 to #3385) audited
  every remaining tmux and xterm.js mention and left these, each with the plan's recommendation:
  - The 65-second "agent has not reported its identity" notice skips Enters inside a paste by scanning typed input for
    xterm.js-style paste markers. The desktop app sends pastes as separate messages, so the scan probably never matches
    now; confirm and remove it.
  - Delete runs an extra scan of every process on the host for every session that was ever restarted, because an earlier
    release's Restart could leave tmux tabs nothing lists; its cost was never measured. Goes with "phase 3".
  - The per-session terminal-kind tracking and the paths that read it, test fixtures that still create old-kind rows by
    default, and about 3,000 lines of test tooling for real tmux servers that now serves only the startup step's four
    tests. Goes with "phase 3".
  - Test-only: a half-second pause before an agent exits in the session lifecycle tests, and an extra error an
    Attach-racing-Delete test still accepts; both existed for tmux, and nobody has checked whether terminal hosts need
    them. Check and remove with "phase 3".
  - Renames when the code is next touched: the "pane" vocabulary across the supervisor and tests,
    `PRIVATE_TMUX_ENV_VARS` (now about the user's own tmux), and test names saying "new terminals" or "never in tmux".
  - Left as they are by the plan: status detection's 16-screen read budget per round and the 10-second prompt delivery
    timeout, both sized for tmux; revisit only if a better value appears.

  Then sweep the repository once more for tmux, xterm.js, pane and old-terminal mentions and remove or reword whatever
  is left that does not belong to "phase 3". Recorded by the maintainer on 2026-11-18 from the plan's review.

- **Cmd-P quick launch sometimes leaves the previous session active.** there is a bug whereby when i use Cmd-P to quick
  launch, sometimes, i'm not sure when, it just creates the session without making it the active session. observed on
  0.39.0. i cannot reproduce consistently, but twice it happened when i had an active codex agent. visigle, at least
  once after having typed in it, and then hitting cmd-p. Findings, 2026-11-18: code reading (no change to the path since
  0.40.0) found no ordinary cause. The maintainer did not recall a "Started" pill, which the designed "moved on" path
  shows. One rare helm-side corner case matches every symptom: the helm's best-effort cache write of a newly created
  session (`record_session`, called from `accept_created_session`) can fail when the host reconnected during the create,
  so the list pushed with the reply is complete but lacks the new session; the desktop app opens it, sees it proven gone
  (`listing_proves_gone`), and reselects the previous session silently. The helm log would then say "could not record
  the session for routing". Nothing ties that to an active Codex agent, so it stays unconfirmed; the maintainer chose to
  keep watching for it.

### Plan follow-up

- **Review the two-lane desktop channel before moving more commands.** Check how the shared queue limit, per-lane
  watchdog and operation keys interact, and verify the boundaries in `Work::lane()`. Connecting N hosts and opening
  Checkout Trash can send up to 2N reads in quick succession. Revisit the choices to stop rechecking after a failed read
  and wait for the pushed epoch after a delete times out. If a post-delete read reports that removal is finished,
  another read is pending when the old delete reply arrives, and that pending read fails, the can can still show removal
  until the next refresh. If the first refresh fails before the delete reply arrives, Trash may show the read failure
  without showing the accepted removal.

### Tensorlake

- **Agent-requested Tensorlake hosts.** Let an agent request a host by naming an existing recipe, behind the user's
  approval.

- **Investigate Tensorlake's ssh gateway leaving forwarded channels open, and report it if it is a bug.** Through the
  gateway, an ssh client holding a remote-forwarded connection never learned that the sandbox side was gone: the sandbox
  side was torn down within half a second, but the local ssh and its connection to the local service were still up 185
  seconds later, with keep-alives on or off, while an ordinary OpenSSH server ended the same setup in about 1.3 seconds
  and the gateway did end a connection with no forwarded channel open. Confirm what the gateway does (including whether
  it relays a forwarded channel's close in the ordinary case where only the sandbox-side client hangs up), and if it is
  a gateway bug, send Tensorlake a report with the minimal reproduction. Everything known so far, measured and inferred
  kept apart, is in `lore/2026-11-07-tensorlake-gateway-forwarded-channels.md`.

- **Skip Claude Code's first-run setup on new Tensorlake hosts.** Decided by the maintainer on 2026-11-07, answering the
  settings-credentials plan's block: Farhelm delivers a selected Claude subscription to a Tensorlake host but writes no
  other Claude configuration, so the first Claude session on a fresh host can stop at Claude Code's own first-run setup
  (its theme chooser and onboarding), which the user goes through once. Claude Code 2.1.318 stopped there in a fresh
  home and also checks a separate onboarding-completion setting. The fix the maintainer deferred: have Farhelm write the
  smallest set of non-secret first-run settings Claude needs, found by an isolated probe, keeping the user's other
  settings and the normal agent home, and check it with a valid subscription login.

### Drift test feature coverage

- **Drift tests: base launch, Codex process isolation, and Cursor/OpenCode interactive turns.** Verify that Codex's
  `--no-daemon` keeps hooks and commands inside the session's process tree and still launches cleanly when hooks are
  disabled, verify Cursor's default launch without a permission flag ("Agent's own setting"), and send a real prompt to
  Cursor and OpenCode to confirm interactive turns work under Farhelm's terminal host (today both stop at the initial
  TUI or trust screen).

- **Drift tests: model selection across all supporting agents.** Add live model-flag coverage for Cursor (`--model auto`
  and catalog models) and OpenCode (`--model opencode/<id>`), cover Goose, Pi, and OMP with both
  `--provider openrouter --model <id>` and no explicit provider, validate current catalog model IDs across Claude,
  Codex, Antigravity, and Muse (rather than only `haiku` or one model), and verify each CLI actually selects the
  requested model rather than silently ignoring it.

- **Drift tests: reasoning effort selection.** Add live coverage for Muse (`--reasoning-effort`) and Goose
  (`GOOSE_THINKING_EFFORT`), which currently have zero drift-test coverage, and exercise non-`low` effort values
  (especially `off`, `minimal`, `xhigh`, and `max`, which vendors rename or drop) across Claude, Codex, Antigravity, Pi,
  and OMP, confirming each CLI accepts and applies the requested effort.

- **Drift tests: permission modes and custom permission flags.** Cover YOLO launch flags on Claude, Codex, Antigravity,
  Grok, Muse, and OMP (currently only Guarded is exercised for those six); Goose's `approve`, `smart_approve`, and
  `chat` modes (`GOOSE_MODE`); OMP's `Approve` mode (`--approval-mode always-ask`); Cursor's "Agent's own setting"; and
  custom permission flags surviving Restart (including Antigravity's `--settings` and `--` argument ordering).

- **Drift tests: initial prompt delivery via launch arguments.** Cover all eight agents that accept a launch-time prompt
  (Claude, Codex, Cursor, Muse, OMP, and Grok via `-- <prompt>`; OpenCode via `--prompt=<prompt>`; Antigravity via
  `--prompt-interactive=<prompt>`), including leading-dash and multiline prompts, proving each starts interactively,
  submits the prompt automatically, does not re-send it on Restart, and works with a named conversation start where
  supported.

- **Drift tests: one-time named conversation starts.** Cover starting a session on a named conversation ID or file
  across all ten agents — especially Cursor's `--resume <chat-id>` and OpenCode's `--session <session-id>`, which have
  no drift coverage today — plus relative and absolute session file paths on Pi and OMP, verifying capture on tracked
  agents and Goose clearing prior capture.

- **Drift tests: conversation capture timing, Stop/UserPromptSubmit hooks, and hooks-disabled mode.** Verify
  `restart_readiness` timing (pre-prompt on Claude and Goose, first-prompt on Codex, Muse, Grok, and Antigravity,
  first-reply on Pi and OMP), end-to-end `Stop` and Grok `UserPromptSubmit` hook payloads through Farhelm's own hooks,
  OMP's 256-byte title-slot prelude, hook invisibility in the TUI (including Antigravity's `{}` stdout), and disabling
  hooks via `FARHELM_AGENT_HOOKS=none` (including Goose's resume-time disable and Codex keeping `--no-daemon`).

- **Drift tests: opt-in persistent hook installation for Grok and Antigravity.** Using isolated config directories so
  the real user home stays untouched, verify that the exact hook files Farhelm installs for Grok (`hooks/farhelm.json`)
  and Antigravity (`plugins/farhelm/{plugin.json,hooks.json}`) are discovered and invoked by the real CLIs inside
  Farhelm to capture conversations, and exit cleanly and silently when the CLIs run outside Farhelm.

- **Drift tests: in-session conversation rotation.** Wire the existing standalone Claude `/clear`, Codex `/new`, and
  Codex `/clear` checks into the drift runner (running Codex `/clear` end-to-end through Farhelm's own hook so pending
  withdrawal and `Stop` confirmation are tested), and add live rotation coverage for Muse (`/clear`, `/new`), Pi
  (`/new`, `/fork`, `/resume`), OMP (`/new`, `/fork`, `/resume`), Grok (`/new`), and Antigravity, proving Restart
  resumes the rotated conversation.

- **Drift tests: subagent and nested-agent report rejection.** Wire Claude's standalone shelled-out `claude -p` test
  into the drift runner and add live coverage for both native subagents and shelled-out nested agents across all six
  foreground-ownership harnesses (Claude, Codex, Muse, OMP, Grok, Antigravity), proving child invocations cannot replace
  or corrupt the foreground session's conversation ID.

- **Drift tests: workspace trust pre-trust settings.** Assert that enabling workspace trust actually prevents the trust
  prompt from appearing on Codex, Muse, and Antigravity (instead of silently answering a prompt that still appeared),
  and add live coverage for Antigravity's per-launch `--settings '{"trustedWorkspaces":[...]}'`, Pi's `--approve` and
  `--no-approve` with a project-local config or extension, Claude's `.claude.json` pre-trust edit in an isolated config
  directory, and Codex's explicit untrusted setting.

- **Drift tests: terminal screen status and dialog detection.** Verify live anchored `Running` and `Idle` readings on
  Claude, Codex, and Muse (catching silent fallback to generic change-counting when vendor TUI layout drifts), live
  workspace-trust prompt detection on an untrusted directory for Claude, Codex, Muse, and Antigravity, and live
  in-conversation `Waiting` approval/question dialogs and `Unknown` overlays (such as model pickers) on Claude and
  Codex.

- **Drift tests: permission-mode status-line footer detection.** Verify Farhelm's live screen-footer classifier reports
  YOLO vs non-YOLO accurately on both YOLO and Guarded launches for Claude, Grok, and Muse, and YOLO on `--force`
  launches for Cursor, catching vendor footer wording or layout drift.

- **Drift tests: `$farhelm` instructions pointer delivery.** Verify all six supporting agents (Claude, Codex, Muse via
  `SessionStart` hook stdout; Goose via MCP `initialize` instructions; Pi and OMP via `--append-system-prompt`) actually
  receive the `$farhelm` instructions pointer in model context, and that disabling instructions omits it without
  breaking conversation hooks.

- **Drift tests: running-agent prompt delivery via bracketed paste.** Verify live Claude and Codex TUIs enable terminal
  bracketed paste mode, and that Farhelm's bracketed-paste plus Enter (Claude) or Tab (Codex) submits cleanly at idle,
  preserves multiline prompts without premature submission, and queues cleanly while busy.

- **Drift tests: environment and recipe credential delivery.** Using isolated Claude (`CLAUDE_CONFIG_DIR`) and Codex
  (`CODEX_HOME`) directories and a local loopback stub HTTP endpoint (zero API cost), verify real Claude and Codex CLIs
  still honor Farhelm's generated credential configuration and send requests to the configured `base_url` with the
  expected auth header.

- **Drift tests: wrapper prefixes and declared command launches.** Verify shell (`bash -c`) and `env` wrapper prefixes
  and declared command launches (`{farhelm_args}`, `{cwd}`, `{codex:trusted-cwd}`) end to end across agents,
  specifically proving conversation capture and Restart survive `exec`-based wrappers on Grok and OMP and resident
  wrappers on Claude, Codex, Muse, and Antigravity.

- **Drift tests: remote `BROWSER` opener integration.** Verify how real agent CLIs on Linux — especially Codex and
  Claude Code, which differ in whether they accept a command template or only a single executable path — invoke
  Farhelm's remote `BROWSER` opener.

### Trivial

- **Dynamic network source for announcements.** add dynamic network source for announcements. we currently have hard
  coded announcements in releases, but i want one where i can push announcements by publishing them centrally and
  deliver to users. details tbd during planning. Plan: `plans/queue/dynamic-announcements.md`.

- **Make registered custom agents visible and consistent in the `farhelm agent` CLI.** Agents currently have no way to
  discover registered custom agent IDs through the CLI. Make custom agents behave like built-in agents in the CLI so
  they are visible to agents as usual: add a way to list registered custom agents (and include them alongside built-in
  types in unknown-agent refusals), reconcile the `--conversation` + `--recipe` disagreement between CLI and templates,
  and restore listing valid built-in choices when `--effort` is mistyped for a built-in agent. Plan:
  `plans/queue/custom-agents-cli-visibility.md`.

## Linux support

- **Tunnels from remote sessions on Linux.** The desktop app opens Cmd/Ctrl-click tunnels (and approval cards' Tunnel
  and open) only in the Mac app; on Linux the pill, the consent switch and the card's Tunnel and open are not offered.
  Port them to the Linux desktop app: the tunnel consent switch in Settings → Terminal, Ctrl-click to tunnel a loopback
  link, the tunnel pill, and the approval card's Tunnel and open, with the same consent and refusal rules as the Mac.

- **Linux installation and updates for the desktop app.** The GPUI desktop app runs on Linux but has no install story:
  no package (tarball, .deb, AppImage or Flatpak not chosen), no desktop entry or icon install beyond what a build
  provides, and no auto-update (the updater runs only in the installed Mac app, so Linux shows no update UI). Decide the
  package form and architectures, how updates arrive (a self-updater, the package manager, or manual), and write the
  install docs and SPEC "Installation and updates"' Linux sentences to match. Known Linux issues to settle with it:
  - No desktop entry or icon is installed, so there is no menu launcher. The app id the entry must match is
    `org.scode.farhelm.desktop`, the Mac bundle id, reused on Linux until decided otherwise. A second launch brings the
    open window forward by forwarding its `XDG_ACTIVATION_TOKEN`, so the launcher must pass one for the compositor to
    allow focus.
  - A launch without a Wayland display refuses with a line on stderr only (Linux has no native alert on that path), so a
    launch from a desktop menu in an X11-only session shows nothing. X11 is not supported at all.
  - No updater and no update UI on Linux: the updater runs only in the installed Mac bundle.
  - The local supervisor is started detached and outlives the app, as on the Mac; whether an installed Linux app should
    run it some other way (a systemd user unit, say) is open. SPEC "Topology"'s Linux NOTE says how the Linux app runs
    and keeps its local supervisor is not specified yet; it needs replacing once this is decided.
  - Under KDE (KWin draws the title bar) the app is untested: no drawn bar, no GPUI Kit frame margin, the 722 by 596
    minimum, and drag, resize and close by KWin's own bar.
  - In a real GNOME session the drawn title bar's drag, double-click maximize and Minimize are untested (headless Mutter
    has no pointer drag and no visible minimize), and a close the compositor asks for (Alt+F4) reaches the app by
    another path than the drawn Close button, also untested.

## Mid term

- **Select-all key for Linux text fields.** Since the emacs-text-field-keys plan, Ctrl+A in a Linux text field moves to
  the start of the line, as on macOS, so Linux text fields have no select-all key (no Edit menu, and fields have no
  context menu). Decide how and whether to have one.

- **Built-in questions tool.** A built-in questions tool in the style of the interactive artifacts: how an agent asks
  the user a structured question (multiple choice, say), presented like those artifacts rather than as plain text.

- Initial version of agent2agent comms.

- **Remove the startup shutdown of the old private tmux server ("phase 3").** The tmux removal plans
  (`tmux-end-old-terminals` and its followers) keep one piece of tmux on purpose: a supervisor that finds the private
  tmux server an older release left running turns it off at startup, marks the sessions that ran in it as interrupted
  and tells the user. Once no installation can still hold such a server, remove that step and what exists only for it:
  the startup code and its tests, the stored terminal-kind column and its reads (a schema change), the old-terminal
  notification kinds, the scrub of `FARHELM_TMUX` from session environments, and the test fixtures that build a server
  in the old layout. Asked for by the maintainer on 2026-11-11 while planning the removal. When that is safe (a release
  count, or the upgrade floor passing the release that ended old terminals) is to be decided when this is picked up.

- **Re-assess letting agents prompt running sessions.** `farhelm agent prompt` (an agent sending a message into another
  session's running Claude or Codex agent) landed and was turned off before any release: the helm refuses every such
  request, and the command is hidden from agents. One gap remains where a delivered message could do something the user
  never agreed to. The "is it waiting on a question?" check uses a status reading up to about two seconds old, so a
  permission or trust question that appears in that window, or right after the agent starts, can receive the message and
  its Enter as its answer. Re-enabling means fixing that gap or deciding to accept it, then setting
  `AGENT_PROMPTS_ENABLED` in `crates/farhelm-helm/src/agent_requests.rs` to true, unhiding the CLI verb, restoring its
  agent instructions and docs, un-ignoring the end-to-end delivery tests, and writing a new changelog fragment. SPEC.md,
  Agent-spawned sessions, keeps the design.

- **Support Kimi Code.** Add Moonshot's Kimi Code CLI as an agent type: terminal launches with guarded and YOLO
  permission flags, model, effort and prompt choices once their command-line contracts are verified, a harness mark and
  docs page, then session tracking — per-launch hooks/instructions, conversation capture for resume, and waiting/status
  recognition.

- **Support DeepSeek's coding agent.** DeepSeek publishes models, but no clearly official terminal coding agent was
  found on 2026-10-28; the DeepSeek CLIs that turn up are third-party. Confirm the product and its binary first, then
  add it the way Kimi Code is to be added.

- **Consider auto-scaling the transfer window.** Uploads, downloads and tunnel relays between the helm and a supervisor
  each have a fixed 4 MiB credit window, so one stream's throughput is capped near 4 MiB per round trip whatever the
  link can carry. Asked for by the maintainer on 2026-11-08 while planning the chunk-size change
  (`plans/queue/mux-chunk-size.md`): consider sizing the window automatically from bandwidth and round-trip time,
  bearing in mind that on remote hosts ssh's own channel window, shared by every stream, may be the real cap.

- **Cap the Mac app's own localhost tunnels.** Recipe tunnels are capped at 64 tunneled connections per host, but the
  relays the helm opens for the desktop app's localhost tunnels take no permit, so their number is unbounded
  (`crates/farhelm-supervisor/src/service/tcp_relays.rs`, around the relay start). Asked for by the maintainer on
  2026-11-08 while planning the chunk-size change.

- **Search sessions by content or action.** Let the user find a session by what happened in it — which session ran a
  certain task, command, or produced a given output — rather than only by name. What gets indexed and how the search is
  exposed are TBD.

- **Make auto-tunneling safe.** Figure out how to make auto-tunneling properly safe: can we use some hostname trick so
  the service does not present as localhost? TBD. Pinggy? TBD.

- **Restore automatic recovery from a withdrawn waiting update.** On 2026-10-14 the signed release advisories plan was
  narrowed to warning about a withdrawn running version and refusing to install a withdrawn release; a withdrawn release
  that is already installed and waiting for a restart now needs manual recovery, and an ordinary relaunch may still open
  it. The plan, with the blocked question and the decision, is
  https://github.com/scode/farhelm/blob/d8f02862ce4014255d02d4383910999b61c85dbb/plans/queue/release-advisories.md.
  Automatic replacement was blocked by what execution found about supervisor startup: a supervisor claims its state and
  migrates its session database before it checks tmux and before it records its running version. If startup fails or
  stops in that window, the database is newer than the record says (or there is no record), so an update check that
  trusts the record could install an older release that then refuses the database. The ordering exists in shipped
  releases (v0.28.2 included) and was reproduced with a migration from schema 30 to 31 followed by a tmux refusal while
  the record still named 0.23.0. The follow-up: have a supervisor durably record the version it is about to run before
  it can migrate any state, handle compatibility conservatively when it cannot be established (existing installations
  and interrupted startups included, without treating old records as trustworthy retroactively), check how commands
  inside sessions select a binary under the new meaning of the record, and then restore the automatic replacement. Start
  with a focused review of supervisor startup and interruption before implementing.

- **Release stable from the approved RC without rebuilding.** When an RC has been built and declared correct, releasing
  stable should default to promoting that RC's artifacts, not rebuilding everything for the stable tag. Today a stable
  is cut from main and the tag-triggered workflow builds every target afresh, so stable binaries differ from the
  approved RC's even when nothing changed. First find out what prevents reusing the RC builds: whether cargo-dist (the
  generated `release.yml`, driven by `dist-workspace.toml`) can publish the RC's artifacts under the stable version, and
  what the release commit, version settlement, curated notes and the maintainer's signing of the six payloads demand of
  the build.

- **Scroll the grid terminal entirely from a back buffer it keeps itself, instead of history sent by the supervisor.**
  The page already keeps the scrollback it shows and scrolling sends nothing
  (`crates/farhelm-ui/assets/grid-terminal.js`, "Scrollback"), but that history arrives as `history` messages from the
  supervisor's emulator, one message per line that scrolled off, each line at the width it was written
  (`crates/farhelm-proto/src/grid.rs`, "History"), and a `history_reset` rebuilds it whenever the emulator resizes.
  Consider dropping that half of the stream: the grid client appends the rows a `frame.shift` pushed off the top to a
  back buffer of its own, keeping each line at the size it was shown at (the terminal's current size, not reflowed by a
  later resize), so scrolling never depends on the supervisor re-reading its emulator. Two constraints decide whether
  that works. Flow control skips screen frames while a client is behind but still sends every line that scrolled off as
  history, which is how it never loses output (SPEC.md), so a buffer fed only by the frames the client saw would have
  gaps; and a client attaching to a terminal that already has output has no local history to show. Both need an answer
  before the shape is settled. Related to, but not the same as, the grid stream's binary encoding (grid-binary-stream,
  landed 2026-11-09), which kept the split and changed how the same lines are encoded.

- **Look into the GPUI frame-source patch further.** The new desktop app carries a patch to its vendored GPUI that keeps
  GPUI's older macOS frame source (`CVDisplayLink`) on macOS 14 and later, instead of the `CAMetalDisplayLink` GPUI uses
  there by default. Without it, a key press took about 55 ms to reach the screen on the test Mac VM, against about 35 ms
  in the old app and about 30 ms with the patch, because the newer source presents a frame about three refreshes after
  it was drawn. The patch costs idle CPU (about 0.4% instead of 0.1%) and gives up macOS choosing the frame rate
  (ProMotion displays, low-power mode). Kept on 2026-11-13; look into whether Apple or GPUI may eventually drop the
  older path, whether the newer source can be made as fast (setting its frame latency to 1 made no difference on the
  VM), and how it behaves on real hardware with a ProMotion display.

- **Revisit drawing inactive windows at full frame rate.** The new desktop app draws a window at full rate even when
  another app is in front, matching the old app, instead of GPUI's default cap of 30 frames a second. That keeps output
  scrolling smoothly in a window you are watching but not using, at the cost of more drawing (and battery) while output
  scrolls in a background window, which is common while agents work. Decided on 2026-11-13 to match today; revisit the
  tradeoff.

- **Revisit the KDE-blur dependency decision.** GPUI's Linux backend pulls in `wayland-protocols-plasma`, a crate
  declared MIT that bundles KDE Wayland protocol XML files licensed LGPL-2.1-or-later, which are compiled into generated
  bindings in the Linux desktop app. GPUI uses it only as a fallback for blurring a window's background on older KDE
  Plasma, and Farhelm never asks for a blurred window. Decided on 2026-11-13: remove it with a small patch to the
  vendored GPUI if that needs no horrible hacks, otherwise accept it as protocol definitions rather than code. Revisit
  whichever was done, and whether the license check should catch GPL-family files bundled inside a crate whose declared
  license is permissive.

- **Command and control drop-down on Command-backtick.** command and control bound to `` command-` `` (drops down)

- **Agent-operable config through command and control.** agent operable config through command and control

### Tensorlake

- **Live resize of Tensorlake hosts.** A host is created at its recipe's size, and a recipe's size change affects only
  hosts created afterwards. Tensorlake can resize a running sandbox in place (CAS image format only): it takes one to
  two seconds and keeps processes running; CPUs go up and down, but memory never shrinks below what the sandbox booted
  with, so a sandbox meant to flex has to boot small. Let the user resize a Tensorlake host (or apply its recipe's
  current size) without recreating it. Phase 1 left this out deliberately.

- **Per-repository warm snapshots for sandbox sessions.** Build on a recipe's prepared snapshot, then refresh each
  checkout with an incremental `git fetch` when a session starts. Decide how these snapshots are refreshed. Keep using
  the recipe's GitHub token delivery to each sandbox for private repositories. Design records:
  `lore/2026-11-03-tensorlake-sandbox-hosts-design.md` ("Phase 2, as far as it was thought through", plus the measured
  platform facts) and `lore/2026-09-29-per-session-sandboxes-as-the-remote-host-goal.md`.

- **Live resize for sandbox sessions.** Let a session's sandbox boot small and grow on demand, preserving its running
  processes. Build on "Live resize of Tensorlake hosts" above; expose it from the session's controls. The first version
  launches at the recipe's size and offers no resize control.

- **Possibly: shared scratch volumes for sandbox sessions.** Explore Tensorlake cloud volumes as scratch space shared by
  sandbox sessions.

- **Possibly: fork a warm sandbox session.** Explore a memory checkpoint or live copy that preserves running processes.
  A copied supervisor retains its identity and the sandbox's boot ID, so an independent fork needs an explicit answer to
  that ownership problem. Ordinary Clone starts a fresh sandbox from the recipe instead.

- **Sandbox-session conveniences left out of the first version.** Recipe recent setups, Save as template from a sandbox
  launch, and bulk Suspend, Resume and Stop and suspend. The maintainer deferred these when simplifying the first
  version on 2026-11-09. Sandbox launches record no recent setups, and Save as template is unavailable for a recipe or
  session-owned host target; recipe templates and individual lifecycle controls are supported.

- **Auto-suspend forgotten sandbox sessions.** A sandbox session bills its CPU and memory for as long as its sandbox
  runs. While the helm is connected, Tensorlake's idle timeout never fires, because the helm's connection counts as
  activity. A session the user forgot about therefore keeps billing until they suspend or delete it. The maintainer
  accepted that for the first version on 2026-11-08 and wants an automatic suspend later, for example after the agent
  has been idle for a while. The rule and its default are to be decided when this is picked up.

- **A UI activity log that feedback can include.** Keep a timestamped log of what the UI did (dialogs, focus, shortcuts,
  requests to the helm and how long they took), and add an "Include application logs" checkbox to Send feedback. The
  point is general: whenever "I did X and something odd happened" or "I did X and it took about 5 seconds" comes up, the
  maintainer files feedback right after noticing, and the log says what actually happened and where the time went,
  instead of an agent guessing from code. The motivating example: a launch typed as `gh:owner/repo` sometimes ended up
  as a session named exactly that in the wrong repository, and code reading could only speculate that the "Rename
  session" prompt after a quick launch took the typing. A log reading "shortcut ⌘P → quick launch; dialog opened: Rename
  session (after quick launch); focus → rename field; typed 11 characters; Enter → submitted" would have settled it in
  one report.

  What to log: semantic events, not raw input. Which shortcut matched and which action it dispatched; dialogs opening
  and closing, with their kind and why they opened where that differs (the post-quick-launch naming prompt versus an
  ordinary Rename); focus moving between named controls; submit, cancel and Escape; launches and other operations
  starting and completing; and every request to the helm with its start, end, outcome and duration. Timing is a
  first-class requirement, not a nicety: every event carries a monotonic timestamp with millisecond resolution (not only
  wall clock), and anything with a duration logs both ends, so "it took like 5 seconds" can be broken down into which
  request, wait or render was slow. Ideally the helm logs the same requests with its own timing under a request id the
  UI log also records, so a slow step can be split into UI, network, helm and host time; how far that correlation goes
  is a design question.

  Privacy, by construction rather than by care: never log typed characters, only counts ("typed 11 characters into
  Name"), since fields hold prompts, paths, repository and host names, and secrets such as API keys. Model events as a
  closed set of typed kinds rather than free-form strings, so a later edit cannot leak a session title or path by
  accident, and name controls by Farhelm's own labels, never by page content or peer-supplied values. Requests are
  logged by route shape, not by the ids, paths or names inside them. The checkbox reveals the exact log text that will
  be sent before Send, keeping the dialog's existing contract that what it shows is the whole submission. A SPEC.md
  "Feedback" update has to state this promise.

  Shape: an in-memory ring buffer per window (last few hundred events or about 10 to 15 minutes, to be tuned), never
  written to disk and never sent anywhere until the box is ticked. This covers the "only noticed afterwards" case as
  long as feedback is filed soon after. Most events can come from a few shared places rather than per-call-site logging:
  the shared modal dialog component, the shortcut dispatcher, one page-wide focus listener, and the API client for
  request timing.

  Caveats:
  - Size. Feedback is capped today at a 4000-character message (shared by the UI, the helm and the farhelm.io endpoint)
    and a 64 KiB request body (the helm and the endpoint); the maintainer plans to raise those limits. Compress the log
    in the client before sending. The dialog still shows the uncompressed text.
  - Where the log lands. The endpoint files each submission as an issue in a private GitHub repository, and an issue
    body is limited to roughly 65,000 characters and is not a place for a compressed blob. The endpoint will need to
    decompress and split the log across comments, or store it elsewhere and link it; decide before building the client
    half.
  - Who can read it. Whoever can see that private repository can read every included log, which is one more reason the
    privacy rules above must hold without exception.
  - The helm side has no file logging yet (stderr only, per SPEC_impl.md), so helm-side timing for a report would have
    to come from an in-memory buffer as well, or wait for file logging.

## Doc todo

- Tell users that a working-copy (checkout) root must be a location only they can write: Farhelm is not designed for
  roots shared with or writable by other local accounts, and does not check. SPEC.md's fresh-checkout section states the
  rule.
- Tell users that closing, exiting, or deleting a terminal tab also stops services the tab shell's startup files started
  there first (an `ssh-agent`, an editor daemon, a detached tmux server), and that services the agent's launch started
  first are not affected. A tmux autostart guarded by `TMUX` fires during the agent's launch, and one that execs tmux
  keeps the agent from starting. Likewise, a personal tmux server or an ssh ControlMaster that the agent is the first to
  start ends with Stop, Restart, Replace or Delete, and one a command in a tab is the first to start ends when the tab
  closes or the session is deleted, each with everything the user later ran in it or through it, including from outside
  Farhelm. SPEC.md's session view section and "Ownership during cleanup and provisioning" state the rules.
- Tell users what Stop, Restart, Delete and closing a tab can and cannot kill on a Mac, and on a Linux host whose
  supervisor runs without a usable systemd user manager. There, a background process that has detached from the
  session's terminal is found only through an environment marker, so one that hides or overwrites its environment
  survives: non-dumpable and setuid programs, and daemons that rewrite their process title, such as nginx with default
  settings or Postgres started through `pg_ctl`. Servers kept in the foreground under the agent are still stopped. Point
  Mac users at running such servers in the foreground, or stopping them themselves. SPEC.md's lifecycle operations
  section states the rule.
- Tell users that bash and zsh are the supported login shells on every host: agents and terminal tabs start through the
  login shell, and other shells, csh and tcsh included, may fail to launch them. SPEC.md's supported user environments
  section states the rule.

## Tricky bugs

## Deflake

- **Wait for the terminal-host helper to restore stdout in the descendant PTY test.**
  `the_exit_is_reported_while_a_descendant_holds_the_pty` in `crates/terminal-host/tests/host_lifecycle.rs` failed in
  the v0.44.1 Linux release gate when it sampled stdout during the helper's readiness FIFO write. Failure evidence
  `0363fbf7-7a8f-4f6d-b2cb-b627e371d917`. The isolated retry `f1492329-18dc-4d14-a6c6-6610282d20f5` and hosted retry job
  `137328954707` passed the test. The fixture should make the descriptor restoration observable before asserting that
  the descendant still owns a PTY.

- **Stop the Tensorlake add test racing the connected credential delivery.**
  `codex_subscription_and_none_enter_setup_without_an_endpoint` in
  `crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs` reads the host's credential delivery record after setup
  and asserts that no Codex settings cleanup was recorded, but the delivery that runs once the new host connects records
  one, so the test fails whenever that delivery finishes first. On the arm64 Linux test worker it failed 2 of 30 runs
  alone on main (`975c7642-2c13-4786-81c5-a15bde4a0d07`) and once in a full helm run
  (`f2abe974-d3aa-4c85-b5e7-855b3e63ac7e`). Assert what setup itself must not do (read or rewrite the agent settings,
  which the fixture's backend events show) rather than the stamp a later delivery writes.

### Difficult deflake

## Broken tests

## Code cleanup

### Turning rules into structure

A 2026-10-19 assessment of about 530 recent product fixes found that most were an agent missing a rule, written as prose
or per-call-site discipline, at one place when a sibling path already followed it. The five refactors chosen from it
landed on 2026-10-21. A second assessment that day found that recurring fixes had since moved mostly to UI state and
interaction. After an adversarial review, the maintainer chose the five behavior-preserving refactors below, each of
which makes one such rule something a type, a single owner or a default enforces. A behavior change aligned with the
problem a refactor targets (making drifted copies consistent, say) is allowed in its own `fix:` PR; unrelated changes
are not.

## Maybe later

- **Park the docs-shot pointer on a quiet point when no element qualifies.** Before each docs screenshot, the pointer is
  parked on an element with an id that nothing reacts to. A redesign that removes the last such element (as the
  launcher's title row went) breaks every shot of that surface until each gets its own park target. Fall back to a point
  in a dialog's padding, a gap between controls or the window margin, clear of every hover and tooltip area and of
  popovers. That needs the vendored GUI bridge (`vendor/gpui-agent`, changed together with its patch file) to hover at a
  coordinate, and on a Mac, where the real pointer moves, the point park has to work too.

- **Restore deleted sessions.** Support undeleting a session that is in the trash. Consider supporting undelete for
  sessions without a managed checkout too, by retaining their metadata.

- **Protect the repository against force pushes and similar, while agents can still merge.** Guard main and the release
  tags against force pushes, deletion and other history rewrites in a way that also binds agents, while still letting
  them open, publish and merge PRs as they do today. Main's branch protection already disallows force pushes and
  deletion, but it does not apply to administrators, and agents work through the maintainer's own GitHub account, so it
  does not stop them. Tags have no protection at all (the repository has no rulesets), though the release procedure says
  a tag is never deleted or reused. Work out what GitHub rulesets, bypass lists or a separate agent identity can enforce
  without blocking the PR flow.

- **Reduce the peak memory of snapshots made mostly of cell fixups.** The terminal snapshot encoder still builds cell
  fixups in their own buffer and copies them after the stream, so a fixup-heavy snapshot peaks near twice its size.
  Painting those bytes in place too needs a separate design; the ordinary stream-heavy case already reuses its final
  blob's allocation.

- **Crappy model warning.** Let the user pick a list of models they never want to run. Warn when an agent's TUI
  indicates that one of those models is being used anyway.

- **Input-method typing in grid terminals.** Terminals that run in a terminal host are drawn by the grid terminal
  client, which does not show an input method's composition at the cursor or place the candidate window there: composed
  text reaches the program once committed, but the composing itself happens out of sight. Doing it the way xterm.js does
  means porting its composition helper (a preview element and an input element kept at the cursor on every render, plus
  Chrome and Korean input that arrives outside composition events), which failed the complexity gate of the
  terminal-host work. Verifying it needs real input methods, not only synthetic composition events.

- **Let an agent delete its own session.** `farhelm agent delete` refuses a session's own id (plan
  `agent-delete-and-prompt`), because the request holds a lock keyed by the asking session that the delete would wait
  on, and the session's credential is revoked mid-request; SPEC.md declines `farhelm agent replace` for the same reason.
  Allowing it needs a shape where the helm answers first and deletes afterwards. Decided 2026-10-16 while planning agent
  deletion.

- **Reconsider an ordered WebSocket channel between the GUI and the helm.** On 2026-10-15 the maintainer chose ordering
  in the GUI instead (a request waits only for this GUI's unanswered saves; plan `ordered-gui-helm-requests`), after a
  planning deep dive. The deep dive found that the helm could replay WebSocket frames into its existing routes
  unchanged, but estimated about 5,000 to 8,000 lines over 7 to 10 PRs. The largest single cost is migrating the browser
  tests, which intercept API calls over HTTP in 461 places that would not see WebSocket frames. It also leaves lasting
  complexity: plain HTTP must stay for tabs left open across a helm upgrade, every new route needs an ordered-or-not
  decision, middleware must be kept in step across two ways into the helm, requests disappear from browser devtools as
  separate entries, and one stalled socket stalls every request. Its one gain over GUI-side ordering is fewer
  connections over a high-latency tunnel. Large transfers (attachments, downloads), terminals and the event feed would
  stay off the channel. Revisit if remote browser use over slow links becomes common, or if GUI-side ordering proves
  insufficient.

- **Further harden the release and update trust chain.** The chain is already hardened: installs and updates come from
  get.farhelm.io with signed checksums, and CI no longer signs. Further steps would raise the bar: a Vercel user that
  holds only the get.farhelm.io project; disconnect the docs project from Git (its automatic deploys are off only by a
  setting in agent-writable `website/vercel.json`); tag rulesets and GitHub immutable releases; lock down the DNS,
  registrar and email accounts; and review the source diff before signing, since the signature proves who published a
  release, not that its code is sound.

- **Experiment: a different allocator for the release binaries.** EXPERIMENT ONLY; nothing changes without a measured
  win. The release binaries are static musl, whose allocator hands memory back to the OS eagerly; about a quarter of the
  supervisor's CPU was the kernel's cross-CPU interrupts from those `munmap` calls, though most of the churn came from
  polling that has since been removed (#1738, #1766). Profile the supervisor again, try mimalloc or jemalloc, and keep
  it only if a before/after measurement on the same workload shows a clear CPU win worth building a C allocator for
  every release target. Baseline numbers and method: `lore/2026-10-08-supervisor-idle-cpu.md`.

- **Install Claude and Codex on a host for the user.** Offer to install Claude Code and Codex (only those two) on a host
  on the user's behalf, always through each vendor's one-line curl installer, and optionally copy the user's credentials
  for them over to the host. Choosing to copy credentials must come with a warning that it is the user's call, including
  the risk that a provider bans the account for use across hosts. Offer it in two places: as an action in the host's
  pop-up menu, and as an opt-in option (off by default) when adding a host. Relatedly, adding a host should probe it
  once Farhelm's own installation there is complete.

- **Guard the website's install command against a rogue change.** Add automated protection so that an agent cannot
  replace the `curl … | sh` install command on the docs website without the maintainer noticing. The command is what new
  users paste into a shell unread, and it appears in three places (the site header, the Install page, and the README)
  that nothing checks today. Agents land changes to main without the maintainer reviewing each one (the plans monitor
  does, for one) and can trigger a website deploy, so a changed command could reach the live site unseen. What the
  protection checks, and where (the website build, a check against the live site, an alert to the maintainer), is to be
  decided when the entry is planned.

- **Brick test a release before signing it.** Today the brick test can only run after the maintainer has signed the
  release and published it at its version path on get.farhelm.io (it does not need `/latest` moved). The signature is
  about security, not about the release being free of bugs, so needing it first is not a bug in itself. But it ties the
  two steps together, and ideally the brick test would not need a signature, so the maintainer could brick test a
  release separately from signing it. Several parts of the test lean on the signature. The installer and the helm only
  fetch releases from get.farhelm.io, which only serves signed ones. The helm checks the signature on the remote-host
  payloads it downloads. The old app's in-app updater checks it against the keys compiled into that old release. And the
  brick test's first check verifies the candidate against the previous release's keys. The trade-off is realism: every
  way around the signature (a mirror, a test-only key, skipping verification) makes the brick test exercise something
  other than what a user's upgrade actually does.

- **Require a systemd user manager on Linux, with no fallback.** Decided 2026-10-02: on Linux, a usable `systemd --user`
  is simply required, and Farhelm has no fallback for working without one; macOS has no systemd and keeps the portable
  process sweep. A Linux host whose user manager is missing, hung or broken is a broken host, like one with an I/O error
  or a full disk, not a mode Farhelm adapts to. What is left is the implementation, not the decision.

  Today there are two fallbacks. A Linux supervisor that finds no usable user manager uses the same sweep macOS uses,
  with the weaker cleanup guarantee SPEC.md's lifecycle operations section describes; a definite "no usable manager"
  answer is cached for the supervisor's lifetime, while a probe that times out is retried a minute later. And since
  #1279, a launch that meets a slow user manager still starts, just without a scope, so that session relies on the sweep
  for good while later launches check again. Both go. A launch or tab open on Linux that cannot get its scope fails
  visibly instead of starting unscoped, and a supervisor that once found the manager unusable checks again rather than
  keeping that verdict. The code saving is small, since the sweep itself is shared with macOS and only the `/proc`
  reader is Linux-specific. The real gain is that a Linux host stops flip-flopping between systemd and no systemd: scope
  use is decided per launch and per tab against a cached verdict, so one session can mix scoped and unscoped processes,
  and a stale "no usable manager" verdict can make cleanup skip a scope that does exist. That mixing breeds edge cases
  like these two, found in review-feedback triage on 2026-10-02 and folded into this entry rather than fixed separately:
  - Delete skips a tab's scope (`delete-skips-scoped-tab-on-stale-verdict.md`). An agent launched without a scope
    because systemd was briefly unreachable, a terminal tab later opened inside one, and a supervisor whose first
    systemd check after a restart failed: Delete then never checks the tab's scope and reports success, and a process
    the sweep cannot see (such as an `ssh-agent` started by the tab's shell startup files) outlives the session with
    nothing left that can find it.
  - A failed create orphans its scope (`create-rollback-orphans-unconfirmed-scope.md`). When a create fails after the
    agent started and systemd does not confirm the scope's kill, the rollback still removes the half-created session if
    the sweep came up clean, so a daemon the sweep cannot see keeps running with no session to Delete. Dropping the
    fallback does not change this by itself, because the trigger is a hung manager at rollback time. Make the rollback
    behave like every other cleanup Farhelm cannot confirm: the create fails visibly and the session is kept, so a later
    Delete can retry once the host is healthy.

  A related case was folded into this entry on 2026-10-11 for the shared rollback site and remedy, rather than scoped
  and unscoped process mixing:
  - A failed database confirmation discards cleanup ownership
    (`failed-confirmation-discard-session-despite-failed-process.md`). When a create's database confirmation fails and
    its process sweep also fails, a confirmed end of the session's terminal host still drops the session record. A
    surviving detached process then keeps running with no session left to Delete. The trigger is a local storage
    failure. Keep the session when cleanup cannot be confirmed, so a later Delete can retry.

  When executing, check that all three cases are actually gone.

  Once the fallback is gone, also assess what the process sweep still has to do on Linux (to look into, not decided,
  2026-10-05). With every launch and tab in a scope, the walk from the process numbers of a session's terminals (their
  hosts' children) has no Linux reason left, since its stated purpose is hosts without tab scopes, and dropping it there
  removes the short process-number reuse race SPEC_impl.md currently accepts as an interim measure. Whether the
  environment-marker sweep should stay as a backstop, for processes that left the scope but kept the session marker (an
  agent running `systemd-run --user --scope`, say), is part of the assessment.

- **Decide what process cleanup promises on macOS.** macOS offers Farhelm no equivalent of a cgroup scope. launchd
  tracks a job by its process group, which `setsid` escapes, and the kernel's coalitions, the closest analogue, are as
  far as we know private API (not verified). So Stop and Delete there rely on the walk down from the terminal's process
  and the environment marker, with the residuals SPEC.md's lifecycle section lists. Work out what Farhelm should promise
  and do on macOS, and how much of that machinery earns its keep: for example whether the walk from the terminals'
  process numbers is worth it, and whether the short process-number reuse races SPEC_impl.md accepted on 2026-10-05 as
  an interim measure should stay accepted for good.

- **Goose foreground ownership.** Keep the basic Goose reporter for now; it does not distinguish a native or shelled-out
  child that inherits the reporter from the foreground conversation. The stricter database-backed implementation is
  preserved at the `goose-capture-complex-2026-09-23` tag for comparison. The research and proposed smaller replacement
  are recorded in [the Goose session-tracking assessment](lore/2026-09-23-goose-session-tracking.md). Revisit only if
  reliable child isolation becomes a product requirement or Goose exposes a direct root/subagent role signal.

- **Pi foreground ownership.** Assess whether native or shelled-out Pi children can replace or withdraw the foreground
  conversation's restart target, then define the smallest admission check that preserves legitimate foreground
  transitions. This is an assessment task, not a claim that every vendor path has been reproduced.

- **OMP foreground ownership.** Assess whether native or shelled-out OMP children can replace or withdraw the foreground
  conversation's restart target beyond the checks now landed in #814, and define any remaining smallest admission check.
  Preserve legitimate foreground transitions and avoid extending the reporter's scope without evidence. The earlier
  cross-harness evidence for this and the Pi entry is preserved in
  [the historical ownership assessment](lore/2026-09-20-harness-conversation-ownership.md).

- Reconsider the first-use configuration experience for `gh:` launches when no working-copy root is configured. The
  first version refuses the launch and points to the CLI command; consider an inline GUI flow on initial use or another
  improvement that makes setup easier. Keep the general preference for CLI configuration of rarely changed settings.

- Reconsider agent parent/child relationships: either remove them or make them useful. Current parent tracking is
  optional, and fleet `agent create`/`clone` do not record the asking session, so the parent filter cannot reliably
  answer which sessions an agent created. This is largely unused complexity today; assess whether useful tracking is
  worth keeping before extending it. The current limitation is explicitly accepted in SPEC.md.

- Close the read side of what attached hosts can learn about the fleet. Acting across hosts already needs the user's
  approval (or the requesting host's "run farhelm commands without asking" setting), but any attached host can still
  read the fleet-wide session and host listings and, through an approved spawn that applies a template, any template's
  full contents including command lines, and through an approved clone any session's command line; SPEC.md's Local
  authority section accepts that only until this lands. The end state: only explicitly trusted environments may
  interrogate other hosts' session and template data. Preserve the eventual ability for agents to orchestrate sessions
  across hosts through an explicitly authorized launch policy, potentially trusted templates, without a card for every
  step.

- Make agent hook installation an explicit step surfaced to the user: tell them which hooks Farhelm installs for which
  agents, and have them accept specific hooks, so Codex launches no longer need `--dangerously-bypass-hook-trust` and
  its `-c` overrides. Today that bypass lets unreviewed hooks in the user's Codex config home and in a trusted
  workspace's `.codex/` run; SPEC_impl.md accepts it only until this step exists.

- Use the per-host yolo safe / sensitive setting (from the Near term entry "Guard against yolo launches on sensitive
  hosts") to decide which hosts appear red in the session list. The launch warnings this entry originally also proposed
  moved into that Near term entry.

- Support agents' native voice modes by piping audio through the helm to the supervisor and onward to the agent. Assess
  feasibility later, including how agents accept audio and what transport or audio-device integration would be needed;
  this entry does not commit to a design.

- Take a pre-upgrade backup of the on-disk state so a release can be rolled back. Rerunning the installer pinned to an
  older `FARHELM_VERSION` swaps the binaries back cleanly, but it does not make a downgrade work: both stores refuse to
  open a database whose `user_version` is above what the binary understands (deliberately — misreading is worse than
  refusing), so any release that bumps a schema (0.3.0 takes both the helm and the supervisor from 14 to 15) leaves the
  older binary unable to open the state it finds. Today the only rollback is a state-directory backup taken by hand
  before upgrading (the 2026-08-31 upgrade kept one next to the old binary), and remote hosts updated by the helm have
  the same problem for their own supervisor state with nobody taking a backup at all. Wanted: the upgrade paths — the
  installer for the local machine, and the helm's host update for remote ones — snapshot the state directory (a copy, or
  SQLite's backup API, taken while the old version is stopped) before the new version first opens it, keep a bounded
  number of such snapshots, and document the restore. Noted 2026-09-03 while cutting 0.3.0-rc.1.

- Consider dropping the race-proofing around host identity, keeping the identity itself. To be clear about what stays:
  the per-install identity the supervisor mints on first run and stores in its own database, independent of hostname and
  address, so a retargeted row or a state directory moved to another machine is recognized as the same install; "never
  silently merge" as a user-visible rule; and the mismatch surfaced with both identities and an adopt choice. What goes
  is the machinery that closes millisecond windows in RECORDING it: the empty-slot-only compare-and-swap in
  `record_first_contact`, the dialed-configuration check inside the same transaction (a retarget straddling a
  handshake), the separate adopt CAS with in-transaction cache purge, the never-reused connection tokens that every
  session-cache and mutation write carries, and the split of one "something is off" situation into three connection
  states (`identity-mismatch`, `duplicate`, `identity-unverified`) each with its own remedy text and re-probe policy —
  about 1,500 lines of helm implementation plus ~2,000 of tests across `store.rs`, `manager.rs`, and `hosts.rs`. The
  replacement is check-and-ask: on connect, compare the reported identity with the stored one; equal or empty means
  record and proceed, different means freeze and ask. The races it stops defending against need a retarget or a second
  first-contact to land within one handshake of another, and even then the consequence is a wrong identity the next
  connect flags as a mismatch, not a merge nobody sees. SPEC_impl.md's "structurally impossible at the storage layer"
  and "SCHEMA invariant" paragraphs under Helm internals would be rewritten to say the check is a check. Write-up:
  https://claude.ai/code/artifact/c3d3a74b-ae55-45c7-b3ca-fe30f9f97432

- Replace `install.sh`'s park/journal/rollback with plain idempotency. The floor to keep: re-running the installer from
  ANY intermediate state converges to a correct install, and no single binary is ever torn — download and extract into a
  staging directory inside `$INSTALL_DIR`, verify, then `mv` each binary into place, which is an atomic rename on one
  filesystem. Keep the outer brace group too; it is two lines and is what makes a truncated `curl | sh` execute nothing.
  What goes is the transactional layer built on top of that: the `mkdir` lock with ownership checks, the
  `PARK`/`INSTALL`/`UNDONE` journal, `rollback_from_journal`, the two rollback branches in the replacement loop, the
  `.old` parking files and the refuse-unless helpers around them — about 220 of the script's 527 logic lines (the other
  ~570 lines of the file are comments). Be honest about the size: the ~300 logic lines that remain are things any
  correct installer needs — target detection with the Rosetta case, prerequisite probing, latest-version discovery,
  `SHA256SUMS` handling, tar member validation, the `--version` cross-check, PATH advice, the closing messages — and
  `test-install-sh.sh` mostly tests THOSE (404, checksum mismatch, malformed archives, versions, prerequisites, the
  closing-message contract, the nothing-outside-`$INSTALL_DIR` diff); only the forced-failure rollback leg goes. Given
  up: a kill between the two macOS binaries' renames leaves one new and one old until the next run (the desktop shell
  finds its sibling by path, so a mismatch shows as a refusal, not silence), and a failure after placement no longer
  restores the previous binaries — re-run instead. A modest cut, worth taking when someone is in the file anyway rather
  than on its own.

- Fly.io Sprites as a host kind: a session backed by a per-second-billed microVM that freezes when idle, with "pause
  this host" in the UI meaning "stop paying for it". Assessed 2026-08-30 against a real sprite; the findings, the code
  mapping (a `HostKind::Sprite` over the existing ssh transport via the sprite CLI's ProxyCommand emulation, a
  provisioning flavor for a host with no systemd and no sftp, a `Paused` host state), the SPEC conflicts to surface, and
  a build order are in `lore/2026-08-30-fly-sprites-as-a-host-kind.md`. The same entry sizes the related "native app
  attaches to a remote helm" mode and the cheaper installed-web-app alternative.

- **Pluggable host creation.** Let users plug in their own scripts that spawn and destroy hosts for sessions to run on,
  rather than Farhelm knowing each provider itself. Related to Tensorlake hosts and the Sprites host-kind entry above,
  which build specific providers in.

- Sandboxed agents with scoped GitLab credentials, especially on Tensorlake hosts and the proposed Fly.io Sprites hosts:
  give an agent a token that reaches only the repositories it works on, minted per sandbox. GitLab rather than GitHub
  because GitLab lets the token creation be automated, while GitHub's scoped tokens have to be created by hand in a
  browser.

- Extend container sessions to macOS container hosts, help users prepare or obtain images, and consider stronger
  isolation beyond the experimental Linux rootless Podman support.

- **"Learn more" links from the GUI to the docs.** Once the official documentation lives at a stable URL, add direct
  "Learn more…" style links in various places in the GUI that point at the relevant documentation page. Which places get
  one is to be decided when this is picked up.

- **In-app intro guide.** Build an introductory guide into the app itself, so a new user can learn Farhelm's core
  concepts and first steps without leaving it. Form and content are to be decided when this is picked up.

- **Consider allowing exactly one UI attached to the helm.** A single GUI attached to the helm is the supported user
  surface, and several concurrent GUIs (browser tabs, the desktop app, other devices) are best effort. Consider going
  further and refusing a second concurrent UI outright, for simplicity: races between clients would stop being possible
  rather than merely unsupported. Part of deciding is what replaces today's multi-client machinery that SPEC.md's
  session view section specifies (one attached client per session, takeover with a displaced snapshot and take-control
  action), including how moving between devices would work, and that agents acting through fleet operations are not a UI
  and would still act concurrently. Came up in review-feedback triage on 2026-10-01.

- **Inject commands into agents from the UI.** A button such as "stop all work and restart" could send a message to a
  running agent. `farhelm agent prompt` holds the delivery path for Claude and Codex but is turned off for now (see
  "Re-assess letting agents prompt running sessions", Mid term); a UI button and delivery to other agents remain
  separate work. See SPEC.md, Agent-spawned sessions.

- **Make copying work out of the box with Codex.** Since Codex 0.157 its fullscreen interface captures the mouse, so a
  plain drag in its prompt box highlights text without copying it; the drag-copy hint plan only explains this to the
  user. Consider a launcher option to turn Codex's fullscreen mode off (`-c tui.fullscreen_transcript=false`, the
  setting `/tui` → Scrollback saves), or another way to make copying work without the user having to know Codex's copy
  key. Decided 2026-10-02 while planning the hint.

- **Whole-app zoom.** The font size plan covers terminal text only. Consider a zoom for the whole UI as well: the
  desktop app's webview has a native zoom Farhelm never uses (Cmd/Ctrl +/− do nothing there today), while on the web the
  browser's own page zoom already works but cannot be driven from a button. The rest of the UI uses about 110 hard-coded
  pixel sizes, so a CSS-based scale would be a refactor. Deferred 2026-10-02 as more work than the terminal control.

- **Fix Mac window restoration across display scales.** On a Mac whose displays use different scaling (for example a
  Retina laptop and a non-Retina monitor), the desktop app can reopen at the wrong size or position and save that wrong
  geometry again on close. The saved size is converted with the new window's scale and the position through the current
  scale (`crates/farhelm-ui/src/desktop/window_state.rs`). SPEC_impl.md's desktop window-persistence paragraph already
  says a real macOS display check is still needed. This needs work and verification on a real Mac with mixed-scale
  displays; do not fix it blind. Deferred in review-feedback triage on 2026-10-11
  (`window-restoration-uses-wrong-display-scale-macos.md`).

- **Let the Mac's supervisor outlive the desktop app.** The local supervisor now keeps running after the app quits, but
  the helm lives in the app, so until the app is reopened the agents' `farhelm` commands that need it (`farhelm spawn`,
  and the `farhelm agent` verbs) fail inside those sessions, and nothing restarts a supervisor that crashes while the
  app is closed or starts one at login. Consider a launchd agent like the Linux systemd units, so that those commands,
  and the skills and command-line features built on them, keep working with the app closed. Deferred 2026-10-04 while
  planning `plans/queue/hook-report-files.md`, which keeps conversation tracking working without a supervisor and states
  this gap in the spec as expected for now.

- **Keep a nested Grok from erasing the session's conversation switch.** A `grok` started inside a Grok session can make
  Restart resume the wrong conversation. Here's the sequence. You are in conversation A in a Grok session and run
  `/new`, which starts conversation B, and Grok's hook saves a report saying "this session is now in B". Before Farhelm
  reads that report, the session's Grok starts another `grok` (through its shell tool, say). That second `grok` runs
  your configured Farhelm hooks too, because Grok's hooks live in its configuration rather than on the launch's command
  line, and its hook saves a report for the same session that replaces the first one. Farhelm then reads the second
  report, sees that it comes from a `grok` nested below the session's own, and refuses it, which is correct, but B's
  report is already gone. The session stays on A, and Restart resumes A although you were in B. In a brand-new session
  whose first report was the one replaced, nothing is recorded, and Restart stays unavailable. Farhelm never records the
  nested `grok`'s conversation; the failure is a stale or missing conversation, never a wrong one.

  The window is short while the supervisor runs, since it reads reports every two seconds, but covers the whole outage
  while it does not (on the Mac, while the app is closed). It exists because each session keeps only its newest unread
  report (Grok keeps two: the one naming the conversation and the newest per-prompt one), and the check that refuses a
  nested report runs when Farhelm reads the file, not when the hook writes it. Reporting over the supervisor's socket,
  before #1594, checked each report as it arrived, so a refused report could never displace a good one. Claude, Codex,
  Pi and OMP are not affected: Farhelm adds their reporter to the one launch it builds, so a copy of them started inside
  a session reports nothing. Unverified: whether B's later per-prompt reports correct the session on their own; they
  only add detail to a conversation already named, so the session may stay on A until the next `/new` or a restart.
  Possible fixes: keep every report as its own file and apply them in order, so a refused report never replaces another
  (the plan ruled out only a capped queue, which would evict reports during a long outage); keep a slot per reporting
  process; or have the hook check its own process chain before writing. The accepted gap is recorded in SPEC_impl.md's
  "Report files". Found 2026-10-05 reviewing the `hook-report-files` plan's report.

- **Investigate harness-native notifications.** Claude Code and Codex can raise desktop notifications themselves, by
  writing terminal escape codes that a supporting terminal turns into a system notification. Claude Code picks the
  channel with its `preferredNotifChannel` setting (in `/config`; values such as `iterm2`, `ghostty`, `kitty` or
  `terminal_bell`) and fires when it needs the user: a permission prompt, or sitting idle waiting for input. Codex does
  it with `notifications = true` under `[tui]` in `~/.codex/config.toml`, and fires when a turn finishes and on approval
  requests. Inside Farhelm those codes reach the supervisor's own terminal emulator, which does nothing with them, so
  they end in Farhelm's terminal rather than a native terminal app. Investigate what Farhelm could do with them: surface
  them as client notifications, use them as a status signal, or nothing. Related to the ntfy.sh entry under Near term.

- **"Phone home" supervisors.** Allow supervisors to come up and register themselves with the helm, instead of being set
  up or pushed from the helm, so hosts show up dynamically as they come and go.

- **Tagging/capabilities for hosts and sessions.** Add tagging/capabilities so an agent can say "do X on a session on a
  host with capabilities Y" rather than naming specific hosts or sessions.

- **Trust zones.** A trust zone is scoped to a project and carries certain credentials; a session in a trust zone may
  spawn other sessions in the same zone, on hosts that allow that zone.

- **Consider a "project" concept that is not a session or a repo.** A project would be an object that lives across
  sessions and can span repositories, owning things that belong to the work rather than to any one session or checkout:
  its own identity, context, skills, a favicon, memory, and similar. Whether Farhelm has such a concept, what it owns,
  and how it is created and selected are all TBD.

- **Make the session list scale past its 500-session ceiling.** Each host lists at most 500 sessions and the helm's list
  across hosts at most 500, by design (SPEC.md, "The list is served and rendered WHOLE"), and the `session-limit-notice`
  plan only tells the user through the rail bell when a ceiling is hit. If people do run that many sessions, make the
  list scale instead. Recorded by the maintainer on 2026-11-18 while planning that notice.

## Unbucketized
