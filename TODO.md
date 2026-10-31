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

## Planned

## Near term

- **Detect and upgrade an outdated tmux on a host.** tmux is meant to be an implementation detail, so it is a bug that
  Farhelm does not notice when the tmux server running a host's sessions is out of date: older than the tmux Farhelm
  would now use there, or a version with known security problems. Detect that, warn the user (more insistently when a
  security issue is known), and offer to upgrade. Upgrading means replacing the running tmux server, which ends every
  session on that host much like a reboot does, so it has to be the user's choice and say so plainly. The same applies
  when a host's tmux binary was upgraded underneath a server still running the old one (a Linuxbrew `brew upgrade`, or a
  Farhelm update that changes the bundled tmux).

- **Tie a late conversation report to its session on tmux 3.8.** tmux 3.8 no longer reports a pane's process id once the
  pane is dead, and the supervisor accepts a hook report only when that id appears among the reporter's parents
  (SPEC_impl.md, the five-step admission). So with tmux 3.8, a conversation report still waiting to be applied when its
  agent exits is refused, and a later Restart can resume the conversation the agent had before it switched (a `/clear`
  right before exiting, say). Every Mac on a current Homebrew runs 3.8. Left open by the tactical fix for Restart, Stop
  and Delete failing on exited sessions under tmux 3.8 (2026-10-30). The review of that fix raised one option: anchor to
  the pane pid the session-hold marks record, but any process running as the same user can write those files, which
  makes them a weaker trust root than tmux, so that is a design decision.

- **Revisit keeping full durability for the host identity under WAL.** Decided on the maintainer's behalf on 2026-10-30
  when the sqlite-wal landing was blocked: SPEC.md's rule that lasting damage is never accepted, however unlikely the
  trigger, was read as taking precedence over the plan's decision that no write keeps full durability. The plan went
  back to make writes whose loss could leave lasting damage fully durable, at least the supervisor's first creation of
  the host identity: under synchronous NORMAL a power loss right after a host's first start could keep the helm's record
  of that identity and lose the supervisor's, leaving the desktop app refusing to start with nothing to restore. The
  other option was to keep NORMAL for every write and amend SPEC.md to allow that window.

- **Bootstrap a Podman image on a remote host.** Automatically bootstrap a Podman image on a remote host, with some
  reasonable defaults, as a supported flow.

- **Revisit containerized sessions writing the checkout's Git hooks.** Decided on the maintainer's behalf on 2026-10-29
  when landing the Podman sandbox plan: it landed with the gap documented (SPEC.md and the docs page's caution) instead
  of closed. A containerized agent can write the checkout's `.git` hooks and configuration and an agent's project
  settings, which can later run with full account access outside the container. The options: make those read-only inside
  the container (recommended), or warn before an ordinary session uses a checkout a container has used.

- **Revisit deferring Muse question recognition.** Decided on the maintainer's behalf on 2026-10-29 for the Muse
  session-tracking plan: it ships conversation resume and working, idle and trust status for Muse, but not recognizing
  questions or approval dialogs during a conversation, because no provider usable without the maintainer's credentials
  could make Muse ask a real question. Until then a waiting Muse session can look idle. The alternative is to supply an
  authenticated test provider (or a documented fixture protocol) so the executor can capture those screens.

- Initial version of agent2agent comms.

- **Crappy model warning.** Let the user pick a list of models they never want to run. Warn when an agent's TUI
  indicates that one of those models is being used anyway.

- **Restore deleted sessions.** Support undeleting a session that is in the trash. Consider supporting undelete for
  sessions without a managed checkout too, by retaining their metadata.

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

- **Search sessions by content or action.** Let the user find a session by what happened in it — which session ran a
  certain task, command, or produced a given output — rather than only by name. What gets indexed and how the search is
  exposed are TBD.

- **Treat the GUI–helm connection as infallible.** Decided by the maintainer on 2026-10-26 during triage of the review
  queue's highest bucket (`TRIAGE_OUTCOMES.md`, `template-delete-lost-reply-loses-undo.md`), in their words: "The UI and
  the Helm fundamentally should have an infallible connection. If the connection in any way fails then (a) for desktop
  that means the app crashes, and (b) for browser that's a full page reload. We need to stop adding complexity and
  causing code review feedback on this topic. _Logically speaking_, we should treat messages going between helm and UI
  as going over an infalliable transport." Nothing the GUI holds survives such a failure; the GUI starts over from what
  the helm reports, the way the 2026-10-22 browser sign-in-expiry rule already works. The point is to stop review
  findings about lost replies, transport errors and unknown outcomes between the GUI and the helm, and to remove the
  complexity that exists only because that link can fail. The work, in order:
  - Settle the behavior for every case and write it into SPEC.md and SPEC_impl.md. Open questions: whether the desktop
    app literally crashes or restarts its interface from scratch (same logical effect, friendlier); the browser with the
    helm down or restarting for an update, and after a laptop sleep, which should show the outage screen and reload once
    the helm answers rather than loop; terminal and event streams dropping; and the GUI's own request deadlines (10 s
    reads, 60 s for host operations that work on another machine, 15 min for trash deletes, all in
    `crates/farhelm-ui/src/api.rs`). Those deadlines either stay only as a watchdog whose expiry means the helm is
    broken, with the helm always answering promptly (for example "accepted" or "the host timed out"), or go away. The
    boundaries to state: the helm's links to supervisors stay fallible, and their timeouts are ordinary helm replies the
    GUI shows; refusals are replies, not failures. Say how this replaces SPEC.md's rule that a desktop action "either
    completes and reports its outcome, or reports that its outcome is unknown", and how it relates to the 2026-10-22
    decisions on browser sign-in expiry and failed GUI saves.
  - Audit the GUI and the helm for complexity that exists only because the GUI–helm link can fail: lost-reply
    reconciliation (number keys and quick-launch marks, SPEC_impl.md), unknown-outcome reports (including the
    template-delete "may have happened" report this same triage chose as a stopgap), retry ladders, fences against late
    replies, and the tests for all of them.
  - Remove it.
  - Re-triage the review-queue items that rest on a lost GUI–helm reply under the new rule, starting with
    `review_feedback_queue/rename-lost-reply-restores-old-shortcut-indication.md` and
    `review_feedback_queue/failed-test-response-does-not-fence-settings-write.md`.
- **Make auto-tunneling safe.** Figure out how to make auto-tunneling properly safe: can we use some hostname trick so
  the service does not present as localhost? TBD. Pinggy? TBD.

- **Recognize Muse Code questions and approvals.** Detect questions and approval dialogs during a conversation from real
  Muse screens. These can currently look idle while Muse waits for an answer. Capturing them needs an authenticated test
  provider or a documented provider fixture protocol that can ask a real question. Plan:
  `plans/queue/muse-session-tracking.md`.

- **Support Kimi Code.** Add Moonshot's Kimi Code CLI as an agent type: terminal launches with guarded and YOLO
  permission flags, model, effort and prompt choices once their command-line contracts are verified, a harness mark and
  docs page, then session tracking — per-launch hooks/instructions, conversation capture for resume, and waiting/status
  recognition.

- **Support DeepSeek's coding agent.** DeepSeek publishes models, but no clearly official terminal coding agent was
  found on 2026-10-28; the DeepSeek CLIs that turn up are third-party. Confirm the product and its binary first, then
  add it the way Kimi Code is to be added.

- **Containerized sessions: Farhelm integration inside the container.** The first, experimental containerized sessions
  (rootless podman) run like a launch with conversation hooks turned off: no hooks, no conversation capture for Restart,
  no `farhelm agent` commands or spawning, no attachments from inside. Add the full integration, restricted so a
  containerized agent cannot ask for a session outside its container.

- **Containerized sessions: Restart.** Containerized sessions cannot be restarted, because Farhelm captures no
  conversation inside the container; Replace starts over in a new container instead. Support Restart (and Restart with)
  by resuming the conversation the agent recorded in the session's container. Needs the conversation capture from the
  integration entry above.

- **Containerized sessions: `farhelm spawn` and CLI flags.** Let `farhelm spawn` and the other CLI entry points request
  a containerized launch directly; today only the launcher, templates and launches copied from another session carry the
  settings.

- **Consider closing tabs on stop for ordinary sessions too.** Stopping a containerized session stops its whole
  container, terminal tabs included, while stopping an ordinary session ends only the agent and leaves its tabs running.
  Consider making ordinary sessions match.

- **Stop containers an agent started through podman before killing processes.** Today Stop, tab close and Delete end a
  podman container that an agent or tab started (`podman run -d`, say) only by killing its processes, because the
  container's host-side helpers (podman's conmon and its network helper, pasta) stay in the scope of whatever started
  them and inherit its `FARHELM_*` markers. Killing conmon that way leaves podman's record of the container stuck
  ("stopping", or "running" with nothing behind it), and only `podman rm -f` clears it; a `--rm` container is never
  cleaned up. Stop such containers through podman first, then run the existing sweep as the backstop. Findings from
  2026-10-28, checked against rootless podman 4.9.3:
  - Find them without scanning every process: read the operation's own scope (`cgroup.procs` of the agent's scope, a
    tab's scope, or for Delete every scope of the session, which Delete already lists by name) and keep the processes
    that are conmon. The scope is the attribution; no environment read is needed, so it also works for a process that
    rewrote its environment.
  - A container's main conmon names the container on its command line (`-c <id>`, `-n <name>`) and carries podman's
    exact storage settings in its `--exit-command-arg` list (`--root`, `--runroot`, `--cgroup-manager`, `--tmpdir`, and
    so on). Run `podman stop` with those settings so a container in non-default storage is still found.
  - A conmon with `--exec` belongs to a `podman exec` session, not to a container the session started: the container may
    be the user's own long-lived one. Never stop that container; ending the exec's conmon (as the sweep already does) is
    the right cleanup there. An exec with a terminal ends by hangup when its conmon dies; one without a terminal does
    not, which is a residual to state.
  - Run podman with the `FARHELM_*` markers removed from its environment. Podman's shared per-user pause process
    inherits the environment of whichever podman command recreates it, and it is shared with every rootless podman use
    on the account. Killing it is safe (verified: podman recreates it on the next command, joining a running container's
    namespaces, or making fresh ones when nothing runs), but a marked pause process gets swept for no reason.
  - Stop in parallel, under a time bound, and treat podman's reported state rather than its exit status as the result:
    `podman stop` can exit non-zero after a killed exec session even though the container stopped. A stop that fails or
    hangs falls through to today's sweep, so the worst case stays today's messy state, never a stuck operation.
  - Which operation stops which containers follows the scope: Stop stops what the agent started, tab close what that tab
    started, Delete everything the session started. Containers started outside Farhelm, by systemd units or the user's
    own shell, are never in a session's scope.
  - Out of reach, unchanged: Docker containers (started by the root daemon, not by the agent's processes) and podman run
    under `sudo` (root's conmon). On a host without a usable systemd user manager there are no scopes; the cheap
    fallback is `podman ps` and checking each listed container's conmon for the session's markers, which costs one read
    per container and covers only podman's default storage.
  - The experimental containerized sessions stop their own container by name; this entry is about containers an agent
    starts itself, in any session.

- **Make the process sweep cheaper where scopes exist.** The stop, tab-close and Delete sweep finds escaped processes by
  reading the environment of every process on the machine (`/proc/<pid>/environ`), which costs one read per process on
  the host (about 900 on a dev box on 2026-10-28, against 5 in one session's scope). Where a launch or tab has its own
  scope, selecting by the scope's `cgroup.procs` covers the same processes for a fraction of the cost. SPEC_impl.md
  calls the marker sweep the backstop that always runs, including where a scope exists, because a scope can be left by a
  process that runs `systemd-run --user --scope` itself; changing that is a SPEC decision, so weigh what the backstop
  still catches against its cost before changing it. Hosts without a usable user manager keep the full sweep.

- **Containerized sessions: localhost tunnels and file links.** Make localhost tunnels reach services listening inside a
  session's container, and make file links in terminal output work for container paths outside the checkout. Until then,
  a tunnel offered for a localhost URL a containerized agent prints reaches that port on the host, not in the container,
  and file links are offered only inside the checkout.

- **Containerized sessions: plain folders.** Consider allowing the Containerized option for "Where: folder" launches,
  which only managed checkouts get today. Git breaks inside the container for a folder that is a git worktree or a
  subfolder of a repository, since only the folder itself is mounted.

- **Investigate how terminal content reaches the helm/UI.** Switching sessions often takes a visible delay. Consider
  having the helm/UI receive terminal output from all sessions, or the recently used ones, so switching becomes instant
  rather than reliant on remote round trips.

- **Release stable from the approved RC without rebuilding.** When an RC has been built and declared correct, releasing
  stable should default to promoting that RC's artifacts, not rebuilding everything for the stable tag. Today a stable
  is cut from main and the tag-triggered workflow builds every target afresh, so stable binaries differ from the
  approved RC's even when nothing changed. First find out what prevents reusing the RC builds: whether cargo-dist (the
  generated `release.yml`, driven by `dist-workspace.toml`) can publish the RC's artifacts under the stable version, and
  what the release commit, version settlement, curated notes and the maintainer's signing of the six payloads demand of
  the build.

- **Custom agents, starting with conversation tracking.** Let users run an agent harness Farhelm has no built-in support
  for (an internal or unreleased tool, say) and still get conversation tracking, so Restart resumes the conversation.
  Farhelm cannot know such a tool's details, so the user brings the integration and Farhelm defines the contract. A
  design sketched on 2026-10-30:
  - The contract is a public, stable farhelm command the user's hook calls (for example
    `farhelm report-conversation --id <ID>`, or reading one JSON document on stdin and picking the id out with a JSON
    pointer), not the report files the built-in hooks write today. Those carry process ancestry, a format version and a
    temp-file-then-rename protocol; making them public would freeze internals and make every user script reimplement the
    ancestry walk. The existing user-installed hook integrations that call `farhelm internal hook` already show the
    shape.
  - Farhelm promises: a valid report becomes the session's resume target, the latest report wins, and repeating an id is
    harmless; a report made while the supervisor is down is recorded as reports are today; the reporter always exits 0,
    prints nothing and bounds its stdin read, sending problems to the session's hook log so a broken hook never shows in
    the agent's interface; outside a Farhelm session it is a logged no-op, so the same hook can stay configured when the
    tool runs by hand.
  - The user promises: the reporter runs as a descendant of the session's agent process (so it inherits the session's
    environment), whether through per-launch arguments or the tool's own configuration; it reports the conversation the
    session's top-level agent is in now, not a subagent's; the id is an opaque string of bounded length with no control
    characters, substituted into the resume command as one argument.
  - The supervisor checks only what it can without knowing the tool: the report came from a process in the session's
    pane, from the current launch, and the id is well formed. It cannot stop a nested agent from overwriting the report
    or check that the id actually resumes; a bad id shows up as a failed resume command in the terminal, and the docs
    should say so plainly.
  - Wiring: command launches and templates already store a command and a resume command containing `{conversation}`. The
    minimal addition is a "custom" agent type a command launch can declare, with generic screen-based status,
    `{farhelm_args}` filled with nothing, conversation captured only through the reporter, Resume allowed with the
    user's resume command, and no "agent never reported" warning, since Farhelm cannot know when a custom agent should
    report. A named custom-agent catalog in the launcher can come later on the same contract.
  - Maintainer decisions first: this contradicts SPEC.md's statements that integrations are not user-authored and that
    Farhelm identifies an agent's conversation only from the harness's own explicit report. Also decide whether Restart
    keeps the old id until a new report arrives or clears it, whether the reporter gets a mode to withdraw the target
    ("no resumable conversation yet"), and whether to offer optional nesting protection (only admit a report when a
    process with a named executable sits between the pane and the reporter).

- **An in-app signal for serious breaking changes.** Release notes alone are easy to miss. Give Farhelm a way to tell
  users inside the app about a serious breaking change in a release they just updated to, rather than relying on the
  release notes. Commands and wrappers starting to run through bash with full shell meaning
  (`plans/queue/command-shell-meaning.md`) is the kind of change that would have used it.

- **Replace harness icons with colored versions.** Replace the harness icons with colored versions so they are more
  easily identified.

## Doc todo

- Tell users that a working-copy (checkout) root must be a location only they can write: Farhelm is not designed for
  roots shared with or writable by other local accounts, and does not check. SPEC.md's fresh-checkout section states the
  rule.
- Tell users that closing, exiting, or deleting a terminal tab also stops services the tab shell's startup files started
  there first (an `ssh-agent`, an editor daemon, a detached tmux server), and that services the agent's launch started
  first are not affected, though a tmux autostart guarded by `TMUX` never fires during the agent's launch, so when no
  tmux server of the user's is running yet, the first tab usually starts and owns it, and with `exec tmux` a detach ends
  the tab and the server. Likewise, a personal tmux server or an ssh ControlMaster that the agent is the first to start
  ends with Stop, Restart, Replace or Delete, and one a command in a tab is the first to start ends when the tab closes
  or the session is deleted, each with everything the user later ran in it or through it, including from outside
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

- **Folder reset stays outside the dropdown viewport in WebKit.**
  `composer folder dropdown preserves keyboard launch and grouped history` in `e2e/tests/sidebar.spec.ts` selected the
  reset option but failed its viewport assertion in sweep run `e0526613-832f-4fba-b023-b38cc064e5c2`. All three isolated
  classification reruns passed: `e3ebe846-ebc4-44da-9a7c-cf6981037ca2`, `eaf03d42-4533-48b6-9fc6-21ffa08944f1`, and
  `7a80d996-edd8-46aa-8d82-f1ca43c6d5c8`. Cause unknown; preserve the keyboard visibility assertion (FLAKES.md,
  2026-10-29).

- **Native tunnel preview exceeds WebKit's case budget.**
  `native tunnel preview keeps ports and actions inside a 650px pane` in `e2e/tests/header.spec.ts` timed out during
  cleanup at its unchanged 60-second deadline in recorder `1fc8687c-1044-4c63-83f9-dfddf8da2604`, then passed unchanged
  in 58 seconds in `9ebdc54c-8229-4787-a96b-19c2836e083e`. The new-terminal-button report records the same case timing
  out twice and passing unchanged in 58.7 seconds. Cause unknown; the narrow timing margin is a hypothesis, not a
  diagnosed fix.

- **Interrupted-card Replace clicks lost under load.** The WebKit test
  `don't ask again on a refused YOLO replace marks the host, then replaces with the override` in
  `e2e/tests/terminal-restart.spec.ts` failed in full browser run `49c74525-36ad-44e1-825b-878edadfd71c`: the Replace
  confirmation click landed but no YOLO question or replace request followed. On unmodified main `9f0fd7e9` it failed
  once in six runs under added load (`61cbd3cf-a2ab-4ebc-9b91-cc801ba932dd`) and passed four of four on a quiet machine.
  A Cancel click in the same card was lost the same way once in run `13808999-b14a-44ef-a7df-b547a8419a62`. Hypothesis:
  the spec's injected sessions use ids without the host-entry prefix #2368 made mandatory, so the UI's background
  session reads get 400s, and one completing at the click re-renders the card (FLAKES.md, 2026-10-20).

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

- **Feedback dialog's disabled Send color on WebKit.** `the dialog refuses blank and over-long messages without sending`
  in `e2e/tests/feedback.spec.ts` keeps a dark-palette color on the disabled Send button through its light-scheme check:
  text color `rgb(155, 165, 184)` instead of `rgb(90, 101, 121)` in full browser runs
  `0a494db0-73b5-462c-8e8d-33510baf9df6` and `8f80714f-9f10-46f5-af67-be60e573be31`, and background `rgb(29, 34, 44)`
  instead of `rgb(233, 237, 244)` in `a4a4ad3b-56c1-426f-8377-149f23f29d12` on a Tensorlake sandbox, all on stacks that
  did not touch the dialog. Alone it passed five of five in `756ea4b1-29d9-4954-b8a0-31d9cae25dee` but failed one of
  three in `7b4a27da-01e2-4721-aef3-c5c6c201ae50`. Hypothesis: WebKit leaves the existing button's style on the dark
  palette after the test switches the emulated color scheme to light (FLAKES.md, 2026-10-30 and 2026-10-31).

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

- Extend containerized sessions to the macOS desktop, help users prepare or obtain images, and consider stronger
  isolation beyond the experimental Linux rootless Podman support. Plan: `plans/queue/podman-sandbox.md`.

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

## Unbucketized
