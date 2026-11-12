# End old terminals and remove tmux and xterm.js from the running product

Written against main at 683a9a72 on 2026-11-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and this plan changes them where its behavior changes.

This is the first of four chained plans that together remove tmux and xterm.js from Farhelm ("phase 2"):

1. `tmux-end-old-terminals.md` (this plan): the running product. A new supervisor turns off the private tmux server it
   finds, sessions that were in old terminals end up interrupted, and the supervisor's tmux driver, the two terminal
   kinds, byte streams and xterm.js in the page all go.
2. `tmux-drop-from-install.md`: setup, the desktop app, provisioning, uninstall, the installer, and the release payloads
   (the bundled musl tmux) and their signing set.
3. `tmux-drop-from-tooling.md`: test tooling, CI, the brick test and agent instructions.
4. `tmux-drop-from-docs.md`: specs and docs not already updated, a comment and rationale sweep, and a final audit.

A later step, "phase 3", not planned yet and tracked by its own TODO.md entry, deletes the startup shutdown this plan
adds (do not confuse it with plan 3 above). Do only this plan's part; later plans own the rest, and each names what it
expects to find.

## The goal

### The maintainer's request, in their words

> plan the work to completely nuke tmux/xterm.js support.

> besides certain narrow helpers that are copied from xterm.js (should be with attribution), the intent is to have no
> trace of them. unless you find something that seems like it's worthy of my attention and is somehow needed. we want to
> not find in 2 months there's some crazy complexity somewhere due to tmux/xterm.js still

> it's not JUST about deleting references to those things, also understanding how the code works today and if we have
> complexity that is only there either because (a) of tmux specific concerns or xterm.js specific concerns or (b)
> complexity just because we had two different types for a while.

> remember we must also actually terminate our farhelm internal tmux that supervisors have. so in that sense, there will
> be SOME tmux behavior still left, until in some future third phase we remove even that logic. But this phase should
> remove all the tmux installation stuff, bundling of tmux in the installation/distribution stuff, all the release stuff
> having to do with it, musl binaries, all the complexity every where except the bare minimum to handle safely and
> cleanly turning off the tmux the supervisor created.

("musl binaries" means the bundled musl build of tmux. The static musl builds of `farhelm` itself stay.)

The TODO.md entry this plan completes (Near term), verbatim at planning time:

> **After tmux and xterm.js are dropped entirely: simplify terminal kind tracking.** While old terminals (tmux, drawn by
> xterm.js) and new terminals (terminal hosts, drawn by the grid) coexist, every layer tracks which kind each terminal
> is, and several pieces exist only so that a new terminal never touches xterm.js or tmux. Once a release stops
> supporting old terminals, every terminal is a host, and this tracking should collapse rather than linger as dead
> branches. [...] The pieces, as of the new-terminals cutover stack: [page `kind` field and `pending`, the xterm-first
> path, `adoptGrid`, `noteStreamKind`, the xterm link adapters; protocol `TerminalKind`, `SessionInfo.agent_terminal`,
> `TabInfo.kind`, `StreamKind`, `Attach.grid`, `?grid=1` and the stream notice; supervisor `terminal_kind`, presence
> tracking, the refusals while an unusable tmux runs, `--new-terminals`, `tmux_terminals()`, the old-terminal
> notification; tests: the browser old-terminal set and the Rust tests that ask for old terminals by name.]

Read the full entry in TODO.md before starting; its list is part of the scope. Its line "Keep a cached `"tmux"` kind
decodable for as long as `helm.db` can hold rows from before the removal" is satisfied once the fields are gone:
`helm.db` holds the kind only inside cached `SessionInfo` JSON, which has no `deny_unknown_fields`, so stale keys are
ignored.

### Background

Farhelm used to run every terminal in a private tmux server per supervisor (`<state dir>/tmux.sock`), drawn in the page
by a vendored xterm.js. Since v0.34.0 new terminals run each in a terminal host (`crates/terminal-host/`), interpreted
by the supervisor's emulator and drawn by the page's grid client. Old sessions kept running in tmux, with a migration
band and an old-terminal notification telling users that an upcoming release will forcibly terminate them. This plan is
that release's change. Read SPEC.md "Terminal experience" and SPEC_impl.md "Terminal host", "Terminal substrate: private
tmux server", "Terminal widget: xterm.js island" and "The grid client" before starting.

### Acceptance criteria

1. **The startup shutdown ("the kept minimum").** A supervisor that holds its state directory's claim, at the point it
   adopts a running private server today (after the store opens, before sessions are reloaded):
   - connects to `<state dir>/tmux.sock` with a plain socket connect, never running a tmux binary. The socket file
     missing, or (on Linux) the connection refused, means no server: it removes leftover `tmux.sock`, `tmux.conf` and
     `.tmux.conf.tmp-*` files. Any other failure to connect (a wedged server with a full backlog answers `EAGAIN` on
     Linux and a refusal on macOS, like a dead socket) is logged and the files are left, so a later start can still
     reach the server; that residual is accepted.
   - Something answering: it takes the server's process from the socket's peer credentials, refusing anything not owned
     by the same user, snapshots that process's subtree, and then ends the server with SIGTERM and, after a bound,
     SIGKILL, and removes the socket and config.
   - Cleanup of old work does not depend on the server answering: for every old-kind row it converts (next bullet), it
     sweeps that agent's processes and recorded launch scopes by the row's stored identity with the existing sweep
     (`SweepTarget::AgentOnly`, the recorded scope units, and the legacy session-only marker shape older agents carry),
     the server's subtree serving only as extra process-root evidence. Old tabs are swept per tab where their ids can be
     found (tab markers in the subtree's pane roots, recorded tab scope units). Never sweep per whole session: a session
     moved to a host has host terminals carrying the same session marker. Daemons of old tabs that cannot be identified
     (the server already gone, or macOS hiding a shell's environment) are left for that session's Delete, which sweeps
     the whole session; that residual is accepted.
   - Whatever happened (a failure is logged, never fatal: the maintainer chose "start anyway and treat old sessions as
     ended", accepting that a Restart could then start a second agent beside a still-running one), sessions are reloaded
     with every row whose agent terminal is of the old kind and whose state is launching, running or stop-requested
     converted to interrupted, the way the reboot conversion does it, its launch artifacts cleaned up. Every old-kind
     row, in any state, reloads with no terminal and its stored address is never looked up as a host's.
   - Every unresolved old-terminal warning of every session is resolved. Each session whose agent was converted gets one
     notification of a new kind (see Decisions) saying its old terminal was ended by this release and that Restart
     starts it fresh in a new terminal, with the link to the docs page on new terminals. That notice stays unread and
     unresolved across later supervisor starts until the user reads or clears it. Sessions whose old agent had already
     exited get no notice; they keep their stored outcome, show no terminal screen (the last screen lived in the server,
     which is gone), and Restart starts them in a new host. That loss, for sessions the old warning never covered, is
     part of the accepted ending; say it in the report.
   - The whole startup step has one overall deadline, below the time the helm allows a restarted supervisor to come back
     during Update; report both numbers.
   - A session whose old terminal was ended restarts into a new, empty terminal host. Restart of a host session keeps
     carrying its scrollback into the new host, as SPEC.md says; only the tmux-to-host move and its tmux-only seed path
     go. Clone and Replace already start new terminals.
   - `FARHELM_TMUX` is dropped from what terminal-host children inherit (installed units keep setting it until their
     next setup or Update). This is the only tmux-related code the running product keeps; it works the same whether or
     not any tmux binary exists on the host.
2. **No other tmux in the supervisor.** The tmux driver and everything around it is gone: creating tmux terminals, the
   kind switch and `--new-terminals`, `supervisor run --tmux`, adoption, the presence model, the sink, output-reap and
   control-client machinery, flow control built on tmux `pause-after`, tabs as tmux windows, Restart's move of an old
   agent into a host with its scrollback and the emulator's seed path that serves only it, the refusals while an
   unusable tmux runs, the old-terminal warning's reconcile, the tmux seams and timeouts. Every match on the terminal
   kind, the terminal enum and the attach backend collapses to terminal hosts. The `terminal_kind` column stays (no
   schema change): the kept minimum reads it on every startup until phase 3. Schemas 32-34 and their fixtures stay,
   because installs from v0.33 or older can still upgrade directly (upgrade floor 0.23.0). The tmux version-policy
   module (the floor, probe and program resolution) stays for plan 2's users, untouched.
3. **No byte streams, no terminal kinds on the wire.** The protocol version is bumped. `StreamKind`, `Attach.grid`,
   `Attached.stream`, the stream notice, `?grid=1`, `TerminalKind`, `SessionInfo.agent_terminal`,
   `SessionInfo.supervisor_creates_host_terminals` and `TabInfo.kind` are gone, with the helm's byte-stream relay, input
   chunking, browser-driven pause and resume, and queue sizing derived from tmux. Surviving bounds keep their values
   with rationales rewritten in their own terms. The desktop clipboard cap (256 KiB, sized for xterm.js's OSC 52) is
   aligned with the grid's clipboard bound so grid copies between 256 KiB and 8 MiB stop being dropped.
4. **No xterm.js in the page.** The vendored xterm.js, its add-ons and their license files are gone, with
   THIRD_PARTY_NOTICES.md's "xterm.js and its add-ons" entry. "Code ported from xterm.js" and
   `grid-ported-code-LICENSES.txt` stay, with every attribution comment. The page mounts the grid client
   unconditionally: no kind field, `pending`, `adoptGrid`, stream-kind chooser, xterm-first path, restart kind plumbing,
   migration band, terminal-kinds display, Shift+Enter merge, xterm link and file adapters, dual selectors or xterm CSS.
5. **Tests.** No Rust or browser test asks for an old terminal. Tests whose subject is tmux or xterm.js are deleted with
   their code; tests whose subject survives run on terminal hosts. The opt-in real-vendor hook audits
   (`crates/farhelm/tests/e2e/real_agent_capture.rs`) are ported to terminal hosts and run (see Decisions). The media
   harnesses (README hero, demo video, docs screenshots, the release shots' test) run on new terminals; nothing is
   re-captured or published.
6. **Docs that state this plan's behavior** are updated with it: the SPEC.md and SPEC_impl.md sections each PR changes,
   and the docs website page "Move sessions to new terminals" (`website/src/content/docs/docs/using/new-terminals.md`,
   URL kept: shipped UIs and stored notifications link to it), rewritten to say old terminals were ended in this release
   and what that means. Its "tmux is still needed" section stays true after this plan (setup, the desktop app and Linux
   provisioning still require tmux until plan 2), so keep it, reworded only as far as this plan makes it wrong; plan 2
   removes it with the requirement.
7. **Bookkeeping**, each removal in the PR that deletes its code: the TODO.md entries listed under Decisions, and the
   review-queue items listed there.
8. Each PR leaves main coherent: nothing on main references something the PR deleted (scripts, workflow steps, test
   names in instruction files, CLI flags in harness scripts), and nothing is added that a later PR of this plan deletes.
   A release may be cut from main after any PR of this stack, and the landing monitor can stop partway through a stack,
   so every PR boundary must be a safe release for users: a helm updated before its hosts, hosts updated before the
   helm, and hosts never updated. In particular the protocol bump lands in the same PR as, or before, the first change
   that stops the page from drawing a byte stream (an older supervisor on the same protocol still sends bytes for its
   tmux terminals).
9. **The brick test keeps passing at every PR boundary.** Its P0 checks read each session's terminal kind from the
   session details this plan removes (`releasing/mac-vm-test/RECIPE.md` steps 4-5, `guest_workflow.py`'s prompts, the
   matching `BRICK-TEST.md` paragraphs). Change those reads in the PR that removes the kinds display, with review-queue
   item `brick-test-requires-hidden-terminal-kind-details`. Its database checks read the `terminal_kind` column, which
   stays. Plan 3 retires the rest (the tmux branch, base-image text).
10. **The installer's update message** stops promising that sessions keep running if it does (`scripts/install.sh`,
    "Your sessions keep running.", pinned by `scripts/test-install-sh.sh`), in the PR that ends old terminals, since an
    update to this release ends old-terminal sessions; so does SPEC_impl.md's desktop-update paragraph that says the
    same. The report names the shipped release announcement `0.37.0-old-terminal-support`
    (`crates/farhelm-helm/src/release_announcements.json`, "a future release will forcibly terminate them") as stale
    from this release on; it stays as shipped, and whether to retire it is the maintainer's call.

## Decisions already made

Settled by the maintainer while planning on 2026-11-11:

- **Shutdown without a tmux binary.** The maintainer prefers it and accepts needing a binary only if unavoidable;
  planning found it is not needed. "we have only ever had one bundled tmux version", so nothing here depends on tmux
  versions.
- **Shutdown failure:** start anyway and treat old sessions as ended. "this seems simplest and should leave the least
  complexity [...] we have already told users for a while the change is coming [...] let's not add complexity, keep it
  simple." Do not add retry state, refusals or a lock-out for this case.
- **Scrollback:** not carried over. "in general minimal complexity. users have been told to restart already [...] we
  spend absolute minimum complexity on this."
- **User notice:** after the fact, one notification per ended session; every unresolved old-terminal warning is
  resolved. Sessions that lost only old tabs get no notice. No warning before Update. The notice uses a new notification
  kind (one variant), not the old-terminal warning's: planning first proposed reusing that kind, but the plan review
  found that resolving every old-terminal warning at each start would then resolve the new notice on the next supervisor
  start before the user saw it. The old kind stays decodable for stored rows. Phase 3 can remove both once no host can
  hold them unresolved.
- **Update removes the bundled tmux, knowingly** (plan 2): the maintainer confirmed it after learning that on most
  remote Linux hosts `tmux` typed inside a session has been Farhelm's bundled copy (its directory is first on sessions'
  `PATH`). Nothing in this plan depends on it.
- **Protocol bump:** accepted. After the helm updates, every remote host shows "Needs update" until updated, as with any
  protocol change.
- **Media:** move the harnesses to new terminals, including the release shots' test
  (`e2e/docs-shots/release-notes.spec.ts`, otherwise frozen; this plan is the task that targets it). Designs unchanged;
  no re-capture or publish (that stays the maintainer's under `.agents/docs-media.md`).
- **Real-vendor hook audits:** port them to terminal hosts and run them with this machine's vendor logins. The
  maintainer: "the executor should just do it, there is nothing preventing this". "prefer claude and use codex
  minimally": iterate with the Claude tests; run the Codex ones as few times as you can, ideally once when their port is
  done. Spending those turns is authorized by this plan; no rule forbids it (the "Agent screens" flow fence covers
  re-capturing screen fixtures only).
- **Kept as Farhelm's own behavior**, with their xterm.js or tmux justifications rewritten: the 12,000-line scrollback
  (drop the compile-time tie to tmux's history limit), the emulator's screen-text format status detection reads, its
  terminal query answers, key and mouse encodings (the checked-in key and mouse tables are contract data), the grid's
  fit including its 14px scrollbar reserve, and its cursor and selection defaults. The tmux-era agent screen fixtures
  stay (frozen).
- **The user's own tmux** is not Farhelm's and stays as it is: no `TMUX`/`TMUX_PANE` passed on, `TERM_PROGRAM=farhelm`,
  the 2026-11-10 cleanup rules for jobs under a user's tmux, and the test
  `a_scoped_stop_leaves_jobs_in_a_session_started_stock_tmux_server`.
- **Reviewer:** a single fresh-context agent on Opus 5.5 at high effort (see Review gate). If it cannot be reached when
  a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default.
- **Size gate:** about 3,500 added lines across the stack (code, tests and docs; deletions, changelog fragments, TODO.md
  and review-queue removals, and lines moved unchanged from one file to another not counted). Over it, block and bring
  it back.
- **TODO.md**, each removed in the PR that deletes its code: the entry above (in the protocol PR, which removes the last
  of the kind tracking); Maybe later "Check upstream xterm.js for two link bugs at the next upgrade." and Tricky bugs
  "Investigate corruption in the Codex input area when typing quickly." (both in the page PR that deletes xterm.js).
  Leave every other TODO.md entry to the later plans.
- **Review queue**, each removed in the PR that deletes the code it concerns (planning classified them as clearly moot;
  re-check each against your diff before removing, and keep any that turn out to concern code that survives):
  `asset-wait-bypasses-pending-kind`, `attach-autostarts-empty-tmux`, `classic-mouse-snapshot-copy-blocks-later-links`,
  `dead-pane-pid-check-drops-first-conversation-report`, `delete-lost-tab-markers-skip-sweep`,
  `direct-grid-test-bootstrap-order`, `growing-final-output-omits-trimming-notice`,
  `host-tab-management-tmux-dependency`, `malformed-pane-fixtures-mask-intended-validators`,
  `migrated-session-tab-kind-contract-conflict`, `migration-note-dismissal-loses-terminal-focus`,
  `migration-switch-test-accepts-dead-old-agent`, `missing-agent-terminal-blocks-surviving-tmux-tabs`,
  `mixed-stack-missing-tmux-cleanup`, `newline-initial-login-directory-transformation`, `old-terminal-query-answer-doc`,
  `oversized-pane-fixture-pulls-history-onto-screen`, `pending-kind-font-premise`,
  `reaping-test-mistakes-missing-sink-for-complete-cleanup`, `retry-capture-test-cannot-distinguish-smaller-read`,
  `secondary-press-suppresses-primary-selection-copy`, `selection-cleanup-after-grid-adoption`,
  `shallow-pause-observer-exceeds-boundary`, `sink-churn-without-sinks`,
  `startup-alternative-live-clone-pane-source-gap`, `startup-surviving-tab-adoption-source-gap`,
  `terminal-narrow-base-zwj-width-source-gap`, `terminal-skin-tone-width-parity-source-gap`,
  `tmux-capture-capacity-exceeds-memory-estimate`, `tmux-empty-error-doc`, `tmux-exit-empty-spec-citation`,
  `tmux-history-seed-loses-redefined-palette`, `tmux-negative-result-observation`,
  `tmux-no-spawn-test-retired-query-doc`, `tmux-refusal-broken-rustdoc-link`, `tmux-test-probe-deadlines`,
  `version-probe-latches-gone-unusable` (the kept minimum uses no version probe), `tmux-presence-false-doc` (once
  presence tracking is gone), `replacement-cleanup-missing-pane-root-source-gap` and
  `initial-oversized-grid-cannot-attach` only if your change makes them clearly moot (planning was unsure; the second's
  supervisor half likely survives, so reword rather than remove if so). Follow `review_feedback_queue/AGENTS.md` for how
  an item is removed. Leave `concurrent-devbuild-copy-outlives-hash-check`,
  `devbuild-run-only-skips-old-payload-authentication`, `no-tmux-lifecycle-witness` and
  `offline-uninstall-live-tmux-history-unresolved` to plan 2. Remove `brick-test-requires-hidden-terminal-kind-details`
  with the brick test's kind reads (acceptance criterion 9).

## Requirement sources

**The user's request:** the quotes and decisions above.

**Binding repository constraints:** SPEC.md and SPEC_impl.md; root `AGENTS.md` ("Frozen material", "Code rules",
"Changelog fragments", "Conventional Commits", "Tests and validation", "TODO.md and FLAKES.md", the Claude Code shell
rule, "Instructions by path" for every path you touch: `.agents/docs-media.md` for the media harnesses,
`website/AGENTS.md` for the docs page, `crates/terminal-host/SPEC.md` "Change policy" if you touch that crate,
`vendor/README.md` if you touch `vendor/`, `releasing/AGENTS.md` for `releasing/mac-vm-test/` and the release workflow,
`.agents/validation.md` for tests and scripts); `.agents/test-authoring.md` for every test change;
`review_feedback_queue/AGENTS.md` for queue removals. When you change a workflow (`dist-build-setup.yml`, hence a
regenerated `release.yml`) or a check or test script, update `.agents/validation.md`'s inventory and
`.agents/run-all-tests.md` in the same change; when you change an instruction file or rename a heading, run
`python3 scripts/check-agents-md.py`. The release workflow is generated: change `.github/dist-build-setup.yml` and
regenerate per `releasing/AGENTS.md`; never hand-edit `release.yml`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- The kept minimum lives where `new_claiming` in `crates/farhelm-supervisor/src/service/core.rs` adopts a server today
  (around 6592-6635). It reuses `hosts::process::peer_process` for the pinned peer identity and the TERM/grace/KILL
  logic in `hosts::process` (`terminate_within`), with the command-line predicate made a parameter, because tmux
  retitles its server and the host predicate (`--socket <path>` at the end of argv) does not fit. Do not port
  `farhelm-teststate`'s `SO_PEERPIDFD` code into the product.
- Agent cleanup follows the stored rows (acceptance criterion 1), the way a Restart or Delete of an interrupted row
  sweeps its prior launch today; the server's subtree adds roots. For tabs, read the markers of each pane root in the
  subtree with the existing environment reader (a tab-marked root gives `SweepTarget::Tab(id)`), plus recorded tab scope
  units. Do not consult host-tab bookkeeping: it does not exist yet at that point in startup.
- The row conversion is the reload's arm for old-kind rows (`reload_sessions_with_hosts`), using the existing
  transitions and `cleanup_launch_artifacts`, the way the reboot conversion does (core.rs around 7012-7086), not a
  separate store write. Keep `terminal_address` as stored: the reservation settlement reads a non-empty address as
  launch evidence. Never look it up again.
- The FARHELM_TMUX scrub is one entry beside the existing scrub sites of `launch::PRIVATE_TMUX_ENV_VARS` and
  `host_launch.rs`'s `NOT_INHERITED`; re-document those as the user-tmux hygiene they now are. The test
  `the_child_gets_the_supervisors_environment_in_farhelms_terminal` currently asserts `FARHELM_TMUX` passes through; it
  changes with the scrub.
- Shared helpers in `tmux.rs` that terminal hosts use (`LaunchExcerptReducer`, `launch_excerpt_of_text`) move out, not
  away. `sweep_tmux_config_temp_files` also removes `.running-version.tmp-*`, which must stay.
- Test footing: before building anything, triage the supervisor unit tests that use `tests::tmux_supervisor` or
  `SupervisorSeams::tmux_terminals()` (about 33) into tmux-subject (delete with their code), already pinned on hosts in
  the e2e suite (delete), and the rest (move to the e2e suite, which runs in-process supervisors with
  `host_program: Some(farhelm_bin())`, where its public and test-seams API suffices). Extend the existing in-process
  stand-in (`hosts::terminal::test_host`) behind the `host_program` seam only if many need crate internals. State the
  triage's numbers in that PR's description. For the e2e suite: tests whose subject is tmux or byte streams are deleted;
  the known-gap tests (an unenterable directory, a 0x0 attach) are ported only where hosts already behave as SPEC says,
  otherwise deleted and reported, never fixed here. Porting `memory_stays_flat_while_a_viewer_is_stalled` to hosts is
  optional: do it if cheap, otherwise record the lost coverage in the report.
- Tests of the kept minimum use a real tmux server built in the old layout (`<state dir>/tmux.sock`, `fh-<id>` sessions,
  agent- and tab-marked windows whose panes carry the `FARHELM_*` markers) with whatever tmux the test machine has, the
  way `a_scoped_stop_leaves_jobs_in_a_session_started_stock_tmux_server` uses a stock tmux. Run them with the recorder's
  existing modes as they are today; plan 3 sweeps the recorder's `--tmux` option.
- Dual-kind leftovers to delete with their partners, or keep only with a reason stated in their own terms: the page's
  catch-up and reveal reasons that exist only for xterm.js (`size`, `chunks`, the write-callback fields), the vestigial
  `tracking`/`forced` inputs of `interceptLinkPress` on the grid, the grid's `options.size` inherited-size exception and
  first-fit special case (exist for `adoptGrid`), the link hover deferral that imitates xterm.js's row redraw, the
  supervisor's output-reap registry and sink lifecycle (exist for a tmux 3.7b abort), and `SupervisorTimeouts`' tmux and
  sink fields. NOT dead: `capture.rs`'s `submits_a_line` stays. The grid input path uses it (`service/grid_stream.rs`,
  the hook warning's first-input mark, which starts the session-tracking notification's clock); only its tmux call site
  (`service/terminals.rs`) goes, and its docs drop the tmux framing.
- Surviving bounds keep their numbers (`TERM_QUEUE_BYTES`, `MAX_TERMINAL_DATA_LEN` if it survives,
  `TERM_WS_MAX_MESSAGE`): rewrite only their rationale. Re-sizing a live bound needs measurement and is out of scope;
  the clipboard cap is the one fix.
- `farhelm-teststate`'s tmux module: keep what the kept minimum's tests and the stock-tmux test use; plan 3 deletes the
  rest. `farhelm-fixtures`' fake-agent flood modes: delete any that lose their last user here.

## Outline

Each PR is coherent on main by itself (acceptance criterion 8). Bookmarks are `plan/tmux-end-old-terminals/<nn>-<name>`.

1. `01-tests-on-hosts`: the test footing above. Supervisor unit tests and e2e tests whose subject survives run on
   terminal hosts; the real-vendor audits ported and run (Claude first; Codex once). No product change. `test:`.
2. `02-no-tmux-creation`: the supervisor can no longer create tmux terminals: `NewTerminals::Tmux`, `--new-terminals`,
   `SupervisorSeams::tmux_terminals()`, `harness_tmux`, `supervisor_process_tmux`, the tmux create, tab and Restart
   targets, and the tests that exist for them. Move every caller first or in the same PR: `e2e/start-stack.sh` and
   `e2e/readme-hero/start-stack.sh` (and the media capture specs that read `__farhelmTerm` or `.xterm-*`, ported to the
   grid), the browser old-terminal set (`e2e/suite-terminals.ts`, the `@old-terminal` tests,
   `terminal-link-clicks.spec.ts`, `terminal-scroll-freeze.spec.ts`, `FARHELM_E2E_SUITE_TERMINALS` in
   `playwright.config.ts`, global setup and the Tensorlake scripts that import it),
   `scripts/test-tmux-pinned-shutdown.sh` with its release-workflow step. Adoption of a running server stays for one PR,
   so production is unaffected (every shipped supervisor already creates hosts). `refactor:` or `test:` as fits; no
   user-visible change.
3. `03-end-old-terminals`: the kept minimum (acceptance criterion 1), its tests (including a dead old agent whose
   detached job survives in its scope, no server answering with identifiable leftovers, a mixed session whose host
   terminals survive, and the ended notice surviving a second supervisor start), the notice, the docs page rewrite, the
   installer's update message (criterion 10), SPEC.md and SPEC_impl.md for it, and Restart of an ended session into a
   new host. `feat!:` with a `breaking` changelog fragment saying old terminals are ended on upgrade.
4. `04-no-tmux-driver`: the rest of acceptance criterion 2, with `supervisor run --tmux` and its caller in
   `scripts/desktop-smoke.sh`, and the review-queue items that concern the deleted code. May be folded into 03 if
   splitting would leave 03 adding code 04 deletes. `refactor:` (or `feat!:` if removing `--tmux` is user-visible; it
   is, for hand-written units: say so in the fragment).
5. `05-grid-only`: acceptance criteria 3, 4 and 9 together, with the protocol bump in this PR (criterion 8: the page
   must not lose its byte-stream renderer while a same-protocol older supervisor can still send bytes). The HTTP
   contract golden file (`crates/farhelm-helm/http-contract/session-list.json`, shared with the UI's decoder tests), JS
   tests and browser specs, the brick test's kind reads, the TODO.md entries (the two xterm.js ones and the entry this
   plan completes). If it is too large to review as one PR, split it so the bump lands in the first part, never after
   the page change. `feat!:`, amending 03's fragment or adding one for the protocol change (hosts need Update).
6. `06-clipboard-cap`: the desktop clipboard cap aligned with the grid's (acceptance criterion 3's last sentence),
   `fix:` with a fragment; fold into 05 if small.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression at each PR, and assess the combined diff at the tip. Expect: `cargo fmt` check, clippy with
`--all-targets` and `-p farhelm --bins`, the touched Rust selections through `python3 scripts/record-test-run.py`
(supervisor unit tests, the e2e modules you changed, the host terminal selections in the inventory), the JS harness
(`cd crates/farhelm-ui/js-tests && node --test`), the grid component suite, the browser specs you changed on Chromium
and WebKit, `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, `dist generate --check` after a
workflow change, `python3 scripts/check-agents-md.py` after an instruction-file change, `dprint check`, the website
build for the docs page, and `python3 releasing/check-changelog.py format`. The full browser suite once at the tip is
justified here: this plan changes how every terminal mounts. Report checks run, reused and skipped.

## Fallbacks

Examples of departures that need the scope reassessment: a retry or lock-out for a failed shutdown, carrying any
scrollback from an old terminal, a warning before Update, a schema change, re-sizing a live bound, a notification kind
beyond the one new kind decided above, porting `farhelm-teststate`'s pidfd code into the product, or keeping any tmux or
xterm.js code path beyond the kept minimum.

If something tmux- or xterm.js-shaped turns out to be genuinely needed (the maintainer's "unless you find something that
seems like it's worthy of my attention and is somehow needed"), keep it, state why in its docs, and list it in the
report's "things you should know". Do not block for it unless keeping it contradicts a decision above.

Decisions to log: the test triage and its numbers, what the kept minimum reads and signals, how old tab ids are found,
each review-queue item kept rather than removed and why, each bound kept and its new rationale, and every review finding
you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tmux-end-old-terminals-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type, `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/tmux-end-old-terminals/<nn>-<short-name>`, as the outline names them.
- Keep each PR reviewable. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too. Splitting a large outline step into several PRs is fine.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`) at high effort,
launched natively when your harness serves that model, otherwise through `$scode-harness-shellout`. If it cannot be
reached when a review is due and is still unreachable after a reasonable retry, block the plan per
`plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. Never substitute another
reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires. For a mostly-deleting PR, also ask the
reviewer to look for what the deletion left behind: dangling references, comments and docs describing removed behavior,
and code whose only reason was the removed path.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the PRs make the TODO.md and review-queue changes the decisions name. Open, not merged. If a
`## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
