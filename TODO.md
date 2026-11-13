# TODO

A running list of things the maintainer wants fixed or built. This is intent, not history: an entry is REMOVED in the
same PR that addresses it, so the file only ever describes what is still wanted. It is not a roadmap and carries no
priorities unless an entry says so itself.

Twelve buckets, assigned by the maintainer: "definite simplification" is complexity the maintainer has decided to remove
— the decision is made, only the work remains; "planned" holds accepted work to implement later and suppresses duplicate
review triage within each item's stated scope; "plan decisions to revisit" holds the calls made while a plan ran or
landed, often on the maintainer's behalf when a plan blocked, each recording the alternative; "near term" is what should
be picked up next; "mid term" is wanted after the near-term work, sooner than "maybe later"; "doc todo" holds
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

## Plan decisions to revisit

- **Revisit raising sandbox-suspend-overlap's size gate to about 1,000 lines.** Decided on the maintainer's behalf on
  2026-11-12 when that plan blocked: its first PR (#3339, the host-scoped Suspend and Resume rule) came to about 600
  changed lines against a gate of about 400 for the whole plan, because the estimate was low rather than the scope
  growing. The plan was told to build the second PR (a launch's wake waiting out a "host is busy" refusal and failing at
  once when its host is suspended again) as planned, with the gate raised to about 1,000 lines for both, roughly 900
  expected. The alternatives were landing #3339 alone and dropping that hardening, or landing it alone and planning the
  hardening separately; both leave a same-host launch racing a Suspend or Resume able to fail needlessly or time out
  misleadingly.

- **Revisit accepting sandbox-suspend-overlap at about 1,215 lines.** Decided on the maintainer's behalf on 2026-11-12
  when that plan blocked a second time, finished, over the raised limit of about 1,000: #3339 is about 600 changed lines
  and the wake hardening (#3341) about 615 against an estimate of 300, mostly the wake logic's comments and unit tests
  and three browser tests. Both PRs were accepted as they are. The alternative was trimming comments and folding unit
  tests first, which would save perhaps 100 to 150 lines, still be over 1,000, and likely shorten the explanation of why
  a stale suspended state right after a Resume is not a new suspension.

## Near term

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

- **Recognize Muse Code questions and approvals.** Detect questions and approval dialogs during a conversation from real
  Muse screens. These can currently look idle while Muse waits for an answer. Capturing them needs an authenticated test
  provider or a documented provider fixture protocol that can ask a real question. Plan:
  `plans/queue/muse-session-tracking.md`.

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
  - Farhelm manages a container session's own container. This entry covers containers an agent starts itself, in any
    session.

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

- **Enforce that one helm owns each supervisor.** SPEC.md (Topology) says the helm that registers a host owns its
  supervisor and every session on it, and that neither moves between helms, but nothing enforces that: a supervisor
  treats every full-authority connection alike, so a second helm can register a host another helm owns and drive its
  sessions. Enforcing it needs each host to store the identity of its owning helm, refuse connections from any other
  helm, and make taking a host over an explicit operation the user asks for. That takeover has to cover what SPEC.md
  already supports implicitly, adding a host again from a brand-new helm whose registry was lost, so that losing the
  helm still never strands a host; and the stored identity has to stay the same across everything that does not change
  the owner (restarts, updates, reconnects, and the desktop app and a standalone helm taking turns on one state
  directory). Asked for on 2026-11-08, when agent commands from sessions that were not on screen turned out to fail
  because the supervisor's routing was built on the assumption that several helms might share one supervisor.

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

- **Deep dive on SQLite compaction settings.** Review how Farhelm's databases handle compaction: WAL checkpointing
  (nothing sets `wal_autocheckpoint`, so SQLite's default of 1000 pages applies, and the supervisor's database module
  says there is no checkpoint policy), auto-vacuum, and page size. Decide whether the long-lived stores need explicit
  settings or a periodic maintenance step as they grow.

### Trivial

## Mid term

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
  request timing. This is separate from the existing desktop client-log route, which forwards webview console errors
  into the helm's own logs and exists for a different job.

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
  - Browser and desktop both need it; unlike the client-log route, this is not desktop-only.

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

- **Failed-create replay test times out waiting for its create reply under load.**
  `service::core::tests::a_failed_create_replays_its_error_and_a_changed_override_conflicts` in
  `crates/farhelm-supervisor/src/service/core.rs` timed out at its 10-second wait for the create reply in two
  consecutive landing rounds' full Rust runs on Tensorlake sandboxes (runs `eaaa34eb` and `d230a97e`); it passes in
  about a second locally (five in a row, run `9f342a7d`). Cause unknown; see FLAKES.md, 2026-11-09.

- **Crash-retry create test's Delete refused when the user manager stays silent.**
  `create_idempotency::a_crash_before_the_outcome_commit_still_replays_the_same_session` in
  `crates/farhelm/tests/e2e/create_idempotency.rs` failed in two consecutive landing rounds' full Rust runs on
  Tensorlake sandboxes (runs `cf6e2b79` and `614967f5`): the reopened supervisor's systemd probe appears to run out its
  15-second bound under load, and Delete then refuses the recorded scope by design. It passes locally and on a lighter
  rerun. The 2026-09-30 FLAKES.md entry fixed the same exposure in another test by giving it a disabled scope manager;
  this test likely needs the same, or a scope manager that answers quickly. See FLAKES.md, 2026-11-09.

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
  as untrusted, go through the grid's `textContent` spans, Dioxus's escaped text or `textContent`, never `innerHTML` or
  `dangerous_inner_html`, and Rust-built page script interpolates only JSON-serialized values. Exploiting the fallback
  would need a new script-injection bug in Farhelm's rendering, or a navigation escape in Dioxus or WebKit. Such script
  would already hold the device secret and could drive the whole helm API, typing into terminals included, so the file
  read adds a quieter route to files rather than new power. It is wanted at all because SPEC.md's "Client hardening"
  holds the native app to a higher bar: proportionate hardening that narrows what a hypothetical flaw could reach.

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

## Unbucketized
