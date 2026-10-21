# TODO

A running list of things the maintainer wants fixed or built. This is intent, not history: an entry is REMOVED in the
same PR that addresses it, so the file only ever describes what is still wanted. It is not a roadmap and carries no
priorities unless an entry says so itself.

Ten buckets, assigned by the maintainer: "definite simplification" is complexity the maintainer has decided to remove —
the decision is made, only the work remains; "planned" holds accepted work to implement later and suppresses duplicate
review triage within each item's stated scope; "near term" is what should be picked up next; "doc todo" holds
documentation work; "tricky bugs" retains unresolved bug reports and their investigation findings; "deflake" gathers
test and harness reliability work, including CI execution and restoring gates; "broken tests" records tests that fail
deterministically, with the failure and the evidence that it predates any in-flight work; "code cleanup" holds
structural code smells from a whole-codebase assessment, grouped by kind with a suggested order; "maybe later" is wanted
but not soon, and may never happen; "unbucketized" is everything not yet sorted, which carries no implication either
way. Within a bucket, no order unless the bucket explicitly says so.

Known product fixes stay in their product bucket. "Difficult deflake" retains unresolved failures and their
investigation evidence within "Deflake"; that placement does not establish that the cause is test-only. Move a diagnosed
product fix out of "Deflake" rather than changing user-visible behavior as a test correction.

## Definite simplification

- **Make the bundled terminal font a hard requirement.** Decided 2026-10-01: JetBrains Mono Nerd Font ships with
  Farhelm, so failing to load it is a broken deployment, like any other bundled asset, not something to survive. Remove
  the fallback in `crates/farhelm-ui/assets/terminal.js` ("Font settling before mount": the 3 s settle deadline,
  mounting in a fallback font, the late font swap and repaint paths, the do-nothing tracker for engines without the Font
  Loading API) and gate terminal mounts on the font the way `mountWhenReady` already gates on the other bundled files.
  The retry against the browser briefly reporting no face before the font is registered may still be needed. Accepted
  cost: in the web UI over a slow link, a terminal shows nothing until the roughly 2 MB font arrives instead of
  appearing in a fallback font after 3 s. Removes the cause of `terminal-font-promise-leak.md` (discarded 2026-10-01).

## Planned

## Near term

- **Crappy model warning.** Let the user pick a list of models they never want to run. Warn when an agent's TUI
  indicates that one of those models is being used anyway.

- **Fix emoji corruption while terminal output streams.** Emoji in agent output, such as status icons beside changed,
  unchanged, skipped, and failed counts, sometimes render as clipped or partial glyphs. Scrolling redraws them correctly
  until the agent emits more output, when they can break again. Diagnose the rendering or redraw path and keep the
  glyphs intact as new output arrives.

- **Restore deleted sessions.** Support undeleting a session that is in the trash. Consider supporting undelete for
  sessions without a managed checkout too, by retaining their metadata.

- **Mark a session suspended.** A "mark suspended" action, or similar, so a known, named session can be kept without
  keeping it running. A suspended session is greyed out in the list but can still be reached by name, for example from
  the keyboard quick switcher. The nearest thing today is Stop followed later by Restart, which ends the agent but keeps
  its terminal tabs running and shows the session as exited, the same as any other exit. Whether suspending also stops
  terminal tabs, what bringing a session back does, and how it looks in the list are details TBD.

- **Upgrade epochs.** Give every release an upgrade epoch, and only promise clean upgrades across one epoch: an
  installation on epoch N updates normally while the latest release is on N+1, and one whose latest release is N+2 is
  told to uninstall and reinstall. A change that needs new logic in the upgrade path itself ships that logic in a
  release that starts a new epoch; once that release has been out long enough (a week, a month), the next epoch may rely
  on it. Covers the desktop app, the helm, and supervisors. Until this lands, SPEC.md's "Upgrade compatibility and
  client scale" forbids upgrade-breaking changes outright; replace that rule with the epoch rule here. The overall plan
  is `lore/2026-10-05-release-brick-protection-plan.md`.

- **Pin the Rust toolchain.** Nothing pins Rust today: there is no `rust-toolchain.toml`, CI's toolchain action installs
  whatever stable is current when the job runs, and a developer machine keeps whatever stable it last updated to. So
  local checks and CI can lint with different clippies without anyone choosing that. On 2026-10-09 CI was on 1.99 and
  the local machine on 1.98.1, and CI's clippy failed on lints the local one did not have yet (fixed in #1742). Pin an
  exact version so local, CI and the release builds agree, and make toolchain bumps deliberate PRs. First check that the
  cargo-dist release build jobs honor the pin: they install Rust only when it is missing and otherwise use the runner's
  own. Once pinned, drop the newest-stable clippy step from `.agents/run-all-tests.md`.

- **Make the Mac installer race-free.** Updating, automatically updating and uninstalling Farhelm.app can race with each
  other and with a running Farhelm. An install can delete the version a starting Farhelm or a session's `farhelm`
  command is about to use. An automatic update can put back an older version over a newer manual install, or reinstall
  an app that was just uninstalled. A killed run can leave a lock behind that silently stops automatic updates. A power
  cut mid-update has no guaranteed outcome. This started as the review finding that an interrupted update followed by a
  second install can delete the version a starting Farhelm runs from; a small pending-version marker was rejected after
  three adversarial reviews showed it closes only that one case. The design to build is
  https://snippets.scode.org/s/farhelm-installer-race-free-design/ (version 6): versions held by shared locks while in
  use and deleted only under an exclusive lock, one installation lock on `~/Applications`, the locked work done by the
  new `farhelm` through folders it opened and checked, an installation id and generation for the updater, flushes before
  renames, and uninstall in today's order. After four review rounds by gpt-6.1-sol, gpt-6-astra and Opus 5.5, all three
  are satisfied with it as a basis for implementation, and the macOS primitives it relies on were checked on a real Mac
  (https://snippets.scode.org/s/farhelm-mac-lock-flush-probe-results/). Still to settle before building: the page's four
  "Decisions for you", and getting there from today's installer (older releases' tools, the old lock folder), which the
  design deliberately leaves out. FAT, ExFAT and network volumes are out of scope.

- **Consider matching Claude's conversation capture to Codex's.** Codex's conversation capture changed recently;
  consider adjusting how Farhelm captures a Claude session's conversation to match. The maintainer knows which change
  this refers to and what matching it would mean; ask before starting.

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

- **Protect the repository against force pushes and similar, while agents can still merge.** Guard main and the release
  tags against force pushes, deletion and other history rewrites in a way that also binds agents, while still letting
  them open, publish and merge PRs as they do today. Main's branch protection already disallows force pushes and
  deletion, but it does not apply to administrators, and agents work through the maintainer's own GitHub account, so it
  does not stop them. Tags have no protection at all (the repository has no rulesets), though the release procedure says
  a tag is never deleted or reused. Work out what GitHub rulesets, bypass lists or a separate agent identity can enforce
  without blocking the PR flow.

- **Consider an upstream xterm.js API for links spanning agent-drawn rows.** Farhelm's `linkHoverText` in
  `crates/farhelm-ui/assets/terminal-links.js` reads the undocumented `extended.urlId` from the cell returned by
  `getCell`, joining consecutive hyperlink rows for the hover mismatch warning (SPEC_impl.md's paragraph beginning "An
  OSC 8 link's hover compares"). A public API would retire this private read and its browser pinning test. Consider
  extending the OSC 8 provider's `_getRangeWithLineWrap` in the 6.1.0 betas: it currently joins only while `isWrapped`
  holds and the urlId matches, but agent TUIs produce consecutive same-tag rows without that flag. Claude closes the
  link at the edge and reopens the same id and URI after indentation; Codex holds an id-less link open across a cursor
  move past indentation, sometimes before the edge. Alternatively expose a cell's link id or a link's full extent
  through the public buffer API. Nothing has been filed upstream; filing is the maintainer's call. Farhelm vendors
  xterm.js unmodified, so an upstream fix arrives only with a stable release.

- **Re-assess letting agents prompt running sessions.** `farhelm agent prompt` (an agent sending a message into another
  session's running Claude or Codex agent) landed and was turned off before any release: the helm refuses every such
  request, and the command is hidden from agents. One gap remains where a delivered message could do something the user
  never agreed to. The "is it waiting on a question?" check uses a status reading up to about two seconds old, so a
  permission or trust question that appears in that window, or right after the agent starts, can receive the message and
  its Enter as its answer. Re-enabling means fixing that gap or deciding to accept it, then setting
  `AGENT_PROMPTS_ENABLED` in `crates/farhelm-helm/src/agent_requests.rs` to true, unhiding the CLI verb, restoring its
  agent instructions and docs, un-ignoring the end-to-end delivery tests, and writing a new changelog fragment. SPEC.md,
  Agent-spawned sessions, keeps the design.

- **Stop treating Restart with as a separate launch surface by default.** Launch-related features keep needing a
  separate decision for Restart with (and sometimes plain Restart and Replace), and agents default to leaving it out:
  planning the "launch guarded from the YOLO confirmation" entry on 2026-10-20, the planner recommended "the session
  launcher only" and called Restart with "a second, separate wiring"; the maintainer chose to include it and asked why
  this keeps happening. Revisit the choices that make Restart with a second implementation of launching, so a launch
  feature covers it unless there is a product reason not to. What the code and history show (main at 2897c914): (1) New,
  Clone, Replace with and quick launch are all one component, the session launcher (`CreateSessionForm` in
  `crates/farhelm-ui/src/list/create_form.rs`, about 8,850 lines), so a launcher feature reaches all four at once.
  Restart with is a separate dialog (`crates/farhelm-ui/src/restart_with.rs`, about 780 lines) mounted by the session
  view, with its own draft state, validation, Enter handling, focus trap and modal isolation (it sits over a live agent
  terminal, so the page behind it is made inert), its own submit (`POST /api/sessions/<id>/restart` with `with` or
  `with_command`, `api::restart_session`) instead of a create, and no retry key. The only code the two share is the
  model, effort and permission controls (`LaunchControls` in `launch_controls.rs`, extracted for Restart with in #976 so
  "two copies would drift apart"). Everything around those controls is per dialog. (2) The YOLO question is wired four
  times: `YoloConfirmation` (`yolo_confirm.rs`) is one component, but each caller owns its own question type,
  answer-taking and resubmit (`LauncherYoloQuestion` in `create_form.rs`, `RestartYoloQuestion` and `take_restart_yolo`
  in `session_view.rs`, and the Replace questions in `list/view.rs` and `session_view.rs`). (3) Fixes have repeatedly
  landed once per surface: the cancel-then-answer race was fixed separately for Replace (#1658), the launcher (#1659)
  and Restart with (#1660) on one day; Enter on choices came to Restart with on its own (#1737); the two dialogs
  diverged on whether a failed or pending model-list fetch makes a saved choice incompatible and on how effort
  compatibility is checked (triage outcomes for #1659's catalog fix and the custom-flags effort fix both had to say
  "apply to Restart with too" or note that Restart with already differs). (4) SPEC.md describes Restart with as its own
  action with its own fixed fields (agent type, host, folder) and its own Enter paragraph, and says it "does not offer
  templates", while Replace with is the launcher with the host held fixed. So the launcher already supports fixed
  fields; why Restart with was built as its own dialog rather than a launcher mode with agent type, host and folder held
  fixed is not recorded (#978's description gives the live-terminal isolation and the resume request, not a reason
  against reusing the launcher). (5) Plain Restart and plain Replace relaunch the stored launch with no editor (SPEC.md,
  Restart; "plain Restart or Replace" keep stored words), so a feature that edits a launch has nothing to attach to
  there; that part is a real product distinction, not an accident. Why agents scope Restart with out: an entry written
  about "the launcher" covers four flows for the cost of one, while Restart with always costs a second wiring with its
  own state, tests and SPEC paragraph, so "launcher only" looks like the cheap literal reading; nothing in the code
  makes Restart with inherit a launcher feature. The queued `plans/queue/ui-session-actions.md` merges the two Replace
  copies, the operation lock and the refusal-flag decoding, and adds Restart and Delete only if they duplicate the
  Replace flow; it does not touch the launcher-versus-Restart-with split. Options to discuss: (a) Restart with becomes a
  mode of the session launcher (agent type, host and folder held fixed, conversation field shown, submit sends a
  restart), so launcher features reach it by default and SPEC.md describes one surface; costs a large change to an
  8,850-line component and to the modal isolation; (b) a shared launch-draft model and controller (choices,
  compatibility rules, YOLO question, Enter rules) that both dialogs drive, with the dialogs reduced to layout; (c) keep
  two surfaces but add a standing rule in SPEC_impl.md or AGENTS.md that every launch-editing feature names its Restart
  with behavior explicitly, plus shared helpers for the pieces that diverged (catalog compatibility, YOLO
  answer-taking). Decide whether plain Restart and plain Replace stay outside any of these, as SPEC.md's "relaunch the
  stored launch" suggests.

- **Move the trash into a hidden folder.** Deleting a managed checkout's last session moves the checkout into
  `farhelm-archived-working-copies` inside the checkout folder (`ARCHIVE_DIR_NAME` in
  `crates/farhelm-supervisor/src/working_copies.rs`; SPEC.md, Managed checkouts, and the ownership statement that names
  it), a visible folder beside the user's own checkouts. Move it into a hidden folder named in the same style as the
  spare checkouts' `.farhelm-spares` (planned in `plans/queue/spare-launch-checkout.md`), for example `.farhelm-trash`;
  settle the exact name with that plan's. Needs: migrating existing trash folders on every host (archived checkouts are
  recorded by path in the registry's `archive_destination`, so the move updates those records, and a same-filesystem
  rename keeps the identity checks intact); the reserved-name handling in the occupancy scan
  (`occupied_related_names_from_entries`) and the nesting and overlap guards that compare against the archive path;
  repository discovery's skip (`repository_discovery.rs`, which skips the archive folder by name); the path-length
  margin (`ARCHIVE_PATH_GROWTH`, 82 bytes, quoted in SPEC.md, changes with the name); and the text in SPEC.md,
  `docs/github-checkouts.md` and the docs page `website/src/content/docs/docs/using/stop-restart-resume.mdx`. Asked for
  by the maintainer on 2026-10-20 while planning the spare checkouts.

## Doc todo

- Tell users that a working-copy (checkout) root must be a location only they can write: Farhelm is not designed for
  roots shared with or writable by other local accounts, and does not check. SPEC.md's fresh-checkout section states the
  rule.
- Tell users that closing, exiting, or deleting a terminal tab also stops services the tab shell's startup files started
  there first (an `ssh-agent`, an editor daemon, a detached tmux server), and that services started for the agent launch
  are not affected. SPEC.md's session view section states the rule.
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
- Tell users that agents and terminal tabs run without the private tmux server's `TMUX` and `TMUX_PANE`, as in an SSH
  login, so shells an agent starts for its own work see no tmux either. A startup file that starts tmux when `TMUX` is
  unset without first checking for an interactive shell (`[[ $- == *i* ]]`) or a terminal can then cut short the
  environment an agent captures from it (Claude Code sources `.zshrc` without a terminal), leaving the agent without
  setup made later in that file, such as a PATH entry; this is the same as running the agent in a terminal outside tmux.
  SPEC.md's "Ownership during cleanup and provisioning" section states the rule.
- Correct the OMP agent page's claim that `npx` launchers are recognized
  (`website/src/content/docs/docs/agents/omp.md`). npm rewrites its own process command line when it starts, so a live
  npm or npx process never shows the command line Farhelm's OMP process-chain check expects, and that recognition cannot
  match anywhere in the chain; it is only covered by shape tests built on the expected command line. Found while landing
  `omp-pane-guard-all-launches`, where a live `npx` run on Linux confirmed the rewrite. Say what actually works for npx
  launches, or make the recognition match what live npm shows.

## Tricky bugs

- Investigate corruption in the Codex input area when typing quickly. In ordinary use, appending exactly
  `include a SPEC.md` to a prompt quickly made the display show `include a SPE` followed by another line containing
  scattered fragments such as `COMMI`, `PR`, and repeated `SPEC` text, with large gaps between them, before submission.
  Later recurrences in the macOS desktop app showed the input itself is corrupted, not only its rendering: typing `SPE`
  submitted `SPECIALLY`. [Investigation findings](docs/codex-input-investigation.md). Believed fixed by opting xterm's
  hidden input textarea out of macOS inline predictive text (`writingsuggestions="false"` in `terminal.js`), since the
  recent fragments (`SPEC`, `SPECIAL`, `SPECIALLY`) are all dictionary completions of `SPE`. The original report's
  `COMMI` and `PR` are not, and fit xterm re-sending retained capitals instead, which this change does not address. The
  entry stays open to gather evidence: there is no reliable reproduction, so the fix is unconfirmed until the symptom
  stays absent in normal use. A recurrence after this change points back to the xterm composition defects the findings
  doc cites.

## Deflake

- **Interrupted-card Replace clicks lost under load.** The WebKit test
  `don't ask again on a refused YOLO replace marks the host, then replaces with the override` in
  `e2e/tests/terminal-restart.spec.ts` failed in full browser run `49c74525-36ad-44e1-825b-878edadfd71c`: the Replace
  confirmation click landed but no YOLO question or replace request followed. On unmodified main `9f0fd7e9` it failed
  once in six runs under added load (`61cbd3cf-a2ab-4ebc-9b91-cc801ba932dd`) and passed four of four on a quiet machine.
  A Cancel click in the same card was lost the same way once in run `13808999-b14a-44ef-a7df-b547a8419a62`. Hypothesis:
  the spec's injected sessions use ids without the host-entry prefix #2368 made mandatory, so the UI's background
  session reads get 400s, and one completing at the click re-renders the card (FLAKES.md, 2026-10-20).

- **Takeover new tab never shows Detached.** The Chromium test
  `a-tab-opened-unaware-of-a-takeover-does-not-take-the-session-back` in `e2e/tests/terminal-reconnect.spec.ts` failed
  in full browser run `13808999-b14a-44ef-a7df-b547a8419a62`: after a takeover, the losing view's newly opened tab kept
  an empty banner for 20 seconds instead of "Detached". It failed once in ten exact repetitions on that tip
  (`ac532d81-0b4c-4999-98ab-2abf6e8ec1de`) and four times in fifteen on unmodified main `e6b50141`, one worker
  (`8d57653f-2ced-4f0d-a6ad-c669a11d4995`), each failure running into the same 20-second wait. The spec and the terminal
  code it covers changed on main that day (#2368, #2403, #2411); the cause is unknown and the failing traces have not
  been examined (FLAKES.md, 2026-10-20).

- **Same-source Replace menu fails to open after YOLO cancellation.**
  `plain Replace keeps a newer same-source key after old header YOLO cancellation` in `e2e/tests/replace.spec.ts` failed
  in WebKit waiting five seconds for the source row's menu in browser run `d01a19ad-811c-4dae-90b8-2c22a70ab473`.
  Isolated classification reruns failed, passed, failed: `213407df-5293-46eb-87ca-d08bbe6b3535`,
  `46df9503-ab98-4587-a36d-c8392c58183d`, `a4bb8764-8cf4-42f0-aeee-d97ad0267184`. A further WebKit repetition failed
  five of ten attempts (`0c25b7f9-40a5-47d2-af94-0c93edf7a331`). Trace and code review identify a product pointer race:
  dismissing the other row's open menu moves its error into that row, shifting the source toggle between pointer press
  and release. Leave the exclusion in place. An explicit test-side close would hide the bug; choosing between stable
  error space and changed dismissal/input ordering needs a product decision (FLAKES.md, 2026-10-17).

- **Browser stack parent-SIGTERM cleanup.** `scripts/test-start-stack-cleanup.sh` left the stack serving, with state and
  processes intact, after killing its spawner with SIGTERM in run `f2355071-3c67-4a7b-ba05-37f853c4a6b3`. The isolated
  repetition `a1b6e9b1-a246-4272-8d7b-89452a3f4c45` passed unchanged. Leading hypothesis (2026-10-01 review, not
  confirmed): the kill landed while `e2e/start-stack.sh` was still in a foreground startup step, and bash defers the
  TERM trap until that command returns, past the check's 20-second wait. One candidate step is the startup session
  create, whose first create probes the systemd user manager under a 15-second bound; nothing yet shows which step it
  was. `start-stack.sh` now prints timestamped lifecycle markers (each startup step, the orphan watcher's decisions,
  when the signal trap actually ran, cleanup stages), and a failed phase dumps the stack's process tree and supervisor
  log tails before its emergency cleanup. On the next failure, compare the watcher's `sent TERM` marker (and the check's
  own timestamped kill of the spawner) with the `signal trap` marker, and look for a long-running foreground command in
  the dump. A missing `sent TERM` line alone is not proof the TERM was not sent: cleanup can kill the watcher between
  its signal and its marker. A PATH `curl` shim delaying startup `POST /api/sessions` by 25 seconds confirmed trap
  deferral of about 24–26 seconds in run `b9d6942f-1800-40f4-b65d-45f4bd078afa`, but all cleanup phases passed. The
  check's 20 iterations also include command latency. This did not reproduce the historical leak; retain the markers and
  investigate a retained failure before treating interruptible startup commands as its fix.

- **Shared-checkout browser timeout.** The Chromium test
  `borrowers retain the checkout until the final stopped session is deleted` in `e2e/tests/github-checkouts.spec.ts`
  exceeds its 60-second limit on the cache change and on rebuilt, clean main (FLAKES.md, 2026-10-08). Exact runs failed
  at both four and eight CPUs; traces show slow terminal typing after the clones finished, but the cause is unknown.
  Per-character probe input has been replaced with one text insertion, preserving the real terminal echo and Enter.
  Fixture restoration now attempts every step even when session or configuration cleanup fails, fixing the later-engine
  Git URL mapping cascade. The original timeout's cause remains unproved; keep its existing deadline and sweep exclusion
  until focused evidence supports removing them.

### Difficult deflake

- Restore the release integration gate and remove the remaining ignored binary-output test when the named Rust flakes
  above are fixed. #382 restored the helm-death test. Binary output was un-ignored on 2026-09-19; it and the stalled
  viewer RSS, degenerate-size READY, and malformed-sentinel cases still block restoring the entire `farhelm` integration
  target in `.github/dist-build-setup.yml`. The replacement-claim case is fixed by the 2026-09-17 deflake stack (#711),
  and the forced-pause helper mismatch was closed as not reproducible on the verified pin (#710) — their status here
  updates when that stack lands, not before. Browser flakes are separate coverage and do not themselves gate that Rust
  target. The integration suite remains available for explicit local or worker validation; ordinary CI and the release
  gate do not run it while this exclusion stands. A single clean combined run cannot establish that these latent
  failures are fixed; retain the release exclusion until the evidence supports reversing it.

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

- **One draft value for the session launcher.** The launcher splits one launch choice across about 95 separately written
  pieces of state, and each handler resets the dependent ones and checks the in-flight lock itself; it was the
  most-fixed file after 2026-10-15, and one handler still skips that check. The draft becomes one value whose only
  mutator applies those rules. Restart with stays out (see "Stop treating Restart with as a separate launch surface by
  default"). Plan: `plans/queue/launcher-draft-model.md`.
- **One modal dialog shell that owns focus.** About twenty dialogs each assemble their own Tab trap, isolation of the
  page behind them, return of focus on close and keyboard-reachable scroll regions, and focus fixes kept landing one
  dialog at a time. Every modal dialog gets isolation and a Tab trap through the shell. Plan:
  `plans/queue/modal-dialog-shell.md`.
- **One owner for closing popovers when their row moves.** Each change that can move a sidebar or host row closes the
  open menu itself, and every new popover kind had to be added at each of those places. Changes report to one owner, and
  the three gaps left open in a code comment are closed. Plan: `plans/queue/popover-anchor-close.md`.

## Maybe later

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
    its process sweep also fails, a successful tmux kill still drops the session record. A surviving detached process
    then keeps running with no session left to Delete. The trigger is a local storage failure. Keep the session when
    cleanup cannot be confirmed, so a later Delete can retry.

  When executing, check that all three cases are actually gone.

  Once the fallback is gone, also assess what the process sweep still has to do on Linux (to look into, not decided,
  2026-10-05). With every launch and tab in a scope, the walk from the process numbers tmux reports for a session's
  terminals has no Linux reason left, since its stated purpose is hosts without tab scopes, and dropping it there
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

- **Native `<dialog>` for the app's modal dialogs.** The restart-with dialog, the rename dialog (`rename.rs`), and the
  session launcher (`list/create_form.rs`, `install_composer_focus_trap`) are each a plain `div` with `role="dialog"`, a
  fixed backdrop, and a keydown-based Tab trap written in JavaScript. The restart-with dialog also marks the rest of the
  page `inert` while it is open and has a capture-phase keydown safety net, because it sits over a live agent terminal
  and three separate reviews found three different ways keyboard focus could escape it into that terminal. Converting
  these dialogs to a native `<dialog>` opened with `showModal()` would hand focus containment, inertness of the rest of
  the page, Escape handling, and top-layer stacking to the browser, and could replace most of that hand-written focus
  code. What a conversion needs: an imperative `showModal()` call after the element mounts (Dioxus renders the element
  but does not open it); a `cancel` event handler that refuses Escape while a request is in flight, matching today's
  busy rule; restoring focus to the triggering button on close, as the current code does; and checking that the desktop
  app's webview supports `<dialog>` and `showModal()` (the shipped macOS app uses the system WKWebView; the Linux
  development build uses WebKitGTK). It departs from the pattern all three dialogs share today, so convert them together
  rather than one at a time. Not urgent: the restart-with dialog's `inert` guard already closes the practical focus
  leaks.

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

- Extend Muse beyond basic terminal launching: integrate per-launch hooks/instructions, capture the correct conversation
  identity for resume, and recognize Muse's waiting/status signals. Built-in `muse` and `muse-yolo` profiles currently
  use generic activity status without hooks or conversation resume; these are Farhelm integration gaps, not established
  limitations of Muse.

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
  `SHA256SUMS` handling, tar member validation, the `--version` cross-check, PATH and tmux advice, the closing messages
  — and `test-install-sh.sh` mostly tests THOSE (404, checksum mismatch, malformed archives, versions, prerequisites,
  the closing-message contract, the nothing-outside-`$INSTALL_DIR` diff); only the forced-failure rollback leg goes.
  Given up: a kill between the two macOS binaries' renames leaves one new and one old until the next run (the desktop
  shell finds its sibling by path, so a mismatch shows as a refusal, not silence), and a failure after placement no
  longer restores the previous binaries — re-run instead. A modest cut, worth taking when someone is in the file anyway
  rather than on its own.

- Fly.io Sprites as a host kind: a session backed by a per-second-billed microVM that freezes when idle, with "pause
  this host" in the UI meaning "stop paying for it". Assessed 2026-08-30 against a real sprite; the findings, the code
  mapping (a `HostKind::Sprite` over the existing ssh transport via the sprite CLI's ProxyCommand emulation, a
  provisioning flavor for a host with no systemd and no sftp, a `Paused` host state), the SPEC conflicts to surface, and
  a build order are in `lore/2026-08-30-fly-sprites-as-a-host-kind.md`. The same entry sizes the related "native app
  attaches to a remote helm" mode and the cheaper installed-web-app alternative.

- Tensorlake sandboxes as a host kind: the same idea assessed 2026-09-01 against a real sandbox, in
  `lore/2026-09-01-tensorlake-sandboxes-as-a-host-kind.md`. Fits better than sprites — a real SSH gateway makes the
  transport farhelm's existing ssh path with zero code changes (binary stdio and sftp both verified), suspend/resume
  preserves running processes under the same boot id, and platform-managed processes replace the missing systemd — at
  the cost of resume needing an explicit `tl sbx resume` (plain ssh refuses a suspended sandbox) and, today, an sshd
  session leak that defeats idle-suspend until the leaked sessions are reaped (evidence in the lore entry; worth
  reporting upstream).

- **Pluggable host creation.** Let users plug in their own scripts that spawn and destroy hosts for sessions to run on,
  rather than Farhelm knowing each provider itself. Related to the Sprites and Tensorlake host-kind entries above, which
  assess building specific providers in.

- Sandboxed agents with scoped GitLab credentials, especially on the Tensorlake and Fly.io Sprites host kinds above:
  give an agent a token that reaches only the repositories it works on, minted per sandbox. GitLab rather than GitHub
  because GitLab lets the token creation be automated, while GitHub's scoped tokens have to be created by hand in a
  browser.

- Containerize agents on remote hosts or locally on the desktop, with Docker, cgroup-based isolation, or something
  similar. The cgroup scopes Farhelm already launches into track and reap an agent's processes; they do not isolate it.

- Replace xterm.js with libghostty via WebAssembly, assessed 2026-09-12 in `lore/2026-09-12-libghostty-assessment.md`.
  Native libghostty on macOS is ruled out there: it owns its own PTY and renders into an NSView, neither of which fits a
  WebSocket-fed terminal inside a webview. The WASM route through `coder/ghostty-web` (xterm.js-compatible API over the
  upstream wasm) is plausible but not a drop-in: `onBinary`, `parser.registerOscHandler`, `buffer.active`, and `refresh`
  are missing, its `write` parses synchronously so the backpressure model in SPEC_impl.md has to be re-derived, and the
  upstream wasm ships only on the rolling `tip` release with nothing stable to pin. Medium to high effort; the payoff is
  dropping the scroll-freeze workaround and getting Ghostty's own grapheme and SGR handling. First step is a one- or
  two-day spike mounting ghostty-web in the island under WebKit.

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

- **Close the desktop window's filesystem read fallback (hardening only).** NOTE: This is defense in depth, not a bug
  fix. No exploitable bug motivated it, and nothing known today can exercise it. The desktop window loads its page and
  assets over Dioxus's private `dioxus://` scheme. Farhelm's embedded-asset handler
  (`crates/farhelm-ui/src/desktop/assets.rs`) claims only the `assets` path segment, and every other path falls through
  to the pinned dioxus-desktop 0.7.10 default (`protocol.rs`'s `desktop_handler`), whose dioxus-asset-resolver
  percent-decodes the path, uses it as is when it names an existing absolute file, reads it, and answers 200 with
  `Access-Control-Allow-Origin: *`. The page itself lives at `dioxus://index.html/`, so script in the window could fetch
  `dioxus://index.html/etc/passwd`, or the user's ssh keys, same-origin. The resolver also `.expect()`s UTF-8 on the
  decoded path, so a request like `dioxus://index.html/%ff` likely panics in the protocol callback, possibly aborting
  the app (unverified).

  Why this is not exploitable today: the only script in that window is Farhelm's own, embedded in the binary. The
  `dioxus://` scheme is an in-process handler, not a listener, so no other program or web page can send it requests (and
  same-account processes, which could read the files directly, are trusted by SPEC.md's threat model anyway). Dioxus's
  navigation handler blocks every in-window navigation after the first load and hands http(s) links to the system
  browser, so no outside page or iframe runs there. Everything untrusted is rendered as text: terminal output, session
  titles and host-reported metadata, which SPEC.md's "Local authority and trust between hosts" says the GUI must treat
  as untrusted, go through xterm.js cells, Dioxus's escaped text or `textContent`, never `innerHTML` or
  `dangerous_inner_html`, and Rust-built page script interpolates only JSON-serialized values. Exploiting the fallback
  would need a new script-injection bug in Farhelm's rendering or xterm.js, or a navigation escape in Dioxus or WebKit.
  Such script would already hold the device secret and could drive the whole helm API, typing into terminals included,
  so the file read adds a quieter route to files rather than new power. It is wanted at all because SPEC.md's "Client
  hardening" holds the native app to a higher bar: proportionate hardening that narrows what a hypothetical flaw could
  reach.

  Why it is not done yet: as of 2026-10-02 every complete fix seems to need patching Dioxus, either through a
  `[patch.crates-io]` fork of dioxus-desktop or dioxus-asset-resolver (the repo's first patched Rust dependency, to be
  re-applied on every Dioxus upgrade) or through an upstream change picked up on a later Dioxus bump. The in-tree
  alternatives do not hold up: claiming top-level directories as handler names fails because Dioxus matches the raw
  first segment before percent-decoding, so `/%65tc/passwd` still reads `/etc/passwd`; wry refuses to re-register the
  `dioxus` scheme, so replacing it means reimplementing Dioxus's page bootstrap and event channel; and a
  Content-Security-Policy through the custom-head hook is partial, engine-dependent on custom schemes, and does not fix
  the panic. When this is fixed, also correct SPEC_impl.md's "no bundle-directory fallback at all", which holds only
  under `/assets/`. Came up in review-feedback triage (`desktop-protocol-filesystem-fallback.md`) on 2026-10-02.

- **Inject commands into agents from the UI.** A button such as "stop all work and restart" could send a message to a
  running agent. `farhelm agent prompt` holds the delivery path for Claude and Codex but is turned off for now (see
  "Re-assess letting agents prompt running sessions", Near term); a UI button and delivery to other agents remain
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

- **Let the Mac's supervisor outlive the desktop app.** Quitting the app stops the Mac's local supervisor while its
  sessions keep running, so until the app is reopened the agents' `farhelm` commands that need a supervisor
  (`farhelm spawn`, and the `farhelm agent` verbs, which also need the helm) fail inside those sessions. Consider
  letting the supervisor keep running after the app quits (the app already reuses a supervisor it finds running), or a
  launchd agent like the Linux systemd units, so that those commands, and the skills and command-line features built on
  them, keep working with the app closed. Deferred 2026-10-04 while planning `plans/queue/hook-report-files.md`, which
  keeps conversation tracking working without a supervisor and states this gap in the spec as expected for now.

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

- **Check upstream xterm.js for two link bugs at the next upgrade.** After part of a wrapped URL is repainted, clicking
  it can still open the old target, and on very long wrapped lines a URL's click area shifts onto the row above. Farhelm
  does not work around non-security bugs in the vendored bundle (SPEC_impl.md, decided 2026-10-10), so when the bundle
  is next upgraded, check whether upstream fixed these and remove this entry.

- **Investigate harness-native notifications.** Claude Code and Codex can raise desktop notifications themselves, by
  writing terminal escape codes that a supporting terminal turns into a system notification. Claude Code picks the
  channel with its `preferredNotifChannel` setting (in `/config`; values such as `iterm2`, `ghostty`, `kitty` or
  `terminal_bell`) and fires when it needs the user: a permission prompt, or sitting idle waiting for input. Codex does
  it with `notifications = true` under `[tui]` in `~/.codex/config.toml`, and fires when a turn finishes and on approval
  requests. Inside Farhelm those codes reach tmux, which drops them unless `allow-passthrough` is on (Farhelm does not
  set it), and past that they would land in Farhelm's terminal rather than a native terminal app. Investigate what
  Farhelm could do with them: surface them as client notifications, use them as a status signal, or nothing. Related to
  the ntfy.sh entry under Near term.

## Unbucketized

- **Check whether Codex sessions run their commands outside the session.** An unverified finding from planning
  `prompt-running-agent` (2026-10-17). Since Codex 0.157, Codex's terminal app starts or attaches to one shared
  background server per machine and runs its conversation there, unless it is launched with `--no-daemon`; Farhelm does
  not pass that flag. Other tools that run Codex report that its tool commands then execute in that shared server's
  environment rather than in the terminal they launched Codex from, which broke their per-session identity
  (automatis-tools/agents-can-communicate#224, mvschwarz/openrig#69). If that holds for Farhelm, commands a Codex
  session runs may not see the environment Farhelm sets up for the session (such as what the `farhelm` command inside a
  session relies on to know which session is asking) and may run outside the session's processes, which Farhelm uses to
  tell which session a process belongs to. Start by checking, on the installed Codex, whether a command run by a Codex
  session that Farhelm launched sees the session's environment and runs under the session's processes, with and without
  `--no-daemon`; then decide whether Farhelm should pass `--no-daemon` or otherwise adapt.
