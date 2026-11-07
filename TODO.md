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

- **Give every session-list hover card the beside-the-bar placement, with a pointer to what it describes.** Only the
  row's own summary card opens to the right of the sidebar today (`crates/farhelm-ui/assets/tooltip.js`,
  `placeSessionCard`, opted into by the row's `data-tooltip-anchor="session-card"`); every other hover in a session row
  — status, agent mark, activity stamp, stale badge, pin, actions — still uses the ordinary above/below tooltip. Make
  them all open beside the bar like that one, and add a second cue beyond position that ties the card to its target: a
  tasteful arrow or pointer of some kind. Exact visuals and mockups are for planning. Plan:
  `plans/queue/row-hovers-beside-list.md`.

- **Generally rethink the session filtering experience.** The list now names its active filters and shows a removable
  host chip; revisit the broader filtering experience.

- **Overhaul the grid stream's encoding and the grid client's history storage.** Grid terminals (terminals in a terminal
  host) get their screen and history from the supervisor as JSON messages, and the page keeps every history line
  decoded, as one JavaScript object per run of styled text plus a style object per distinct color pair. Measured in
  WebKit (the Mac desktop app's engine) at the 12,000-line scrollback cap, 200 columns, growth in resident memory once
  settled: typical output 44 MiB (xterm.js 65), a different 256-color color in every cell 364 MiB (xterm.js 408), a
  different true color in every cell 737 MiB (xterm.js 502). The benchmark behind those numbers was never committed. On
  2026-11-03 the maintainer asked for the wire encoding and the in-memory representation to be overhauled together so
  that neither is hideously inefficient, and judged making history compact without changing the encoding a waste of
  time. The shape discussed: one compact binary encoding for the grid stream, which the page stores history in nearly as
  it arrives, decoding only the rows on screen and the rows that selection, copying, word and link lookups touch.
  Drawing (one element per visible row) stays as it is, and compressing older history is something to measure afterwards
  rather than assume. The grid stream is not a permanent format, so a new encoding is an ordinary protocol bump
  (`crates/farhelm-proto/src/grid.rs`, "Why this shape", which chose JSON for readable traces and to avoid a
  hand-written binary decoder in the browser, the place where bounds would be easy to get wrong). Commit a benchmark
  that measures memory, attach size and attach time before and after.

- **Revisit exact Ghostty matching for emoji variation selectors.** Decided by the maintainer on 2026-11-04, answering
  the vs16-emoji-width plan's block: new terminals follow Ghostty's widths for U+FE0F and U+FE0E except in two cases,
  pinned in tests. At the last column with autowrap off, Farhelm cannot tell an empty cell from a space the program
  printed (its saved snapshots store both the same way), so it treats a printed space as empty and a selector can widen
  a character Ghostty leaves narrow. On a terminal one column wide, today's wide-character handling stays, so after a
  too-wide character and a pending wrap the cursor can end on a different row than Ghostty's. Matching Ghostty exactly
  needs new saved cell state, a new snapshot format with a fallback for existing snapshots, and changed one-column
  handling.

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

- **Restore deleted sessions.** Support undeleting a session that is in the trash. Consider supporting undelete for
  sessions without a managed checkout too, by retaining their metadata.

- **Deleting checkouts from the trash is far too slow.** Each delete costs a multi-second round trip and blocks its
  caller. Speed it up by doing the deletes concurrently, move them into the background so the UI is not blocked, or
  both. Plan: `plans/queue/instant-trash-delete.md`.

- **Show the user when the trash cannot finish freeing space.** With `instant-trash-delete`, a deleted checkout leaves
  the trash at once and the host removes its files in the background, retrying after restarts when it cannot (files
  owned by a container's root user, say). Decided on 2026-11-07 while planning it: for now such a folder only gets a
  logged warning, and nothing may get stuck or break because of it. Consider improving how visible this is to the user.

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

- **Podman rigor: review findings for containerized sessions.** Future work for the experimental feature, from the
  2026-11-04 triage; these findings were assessed from source, not reproduced at runtime.
  - Stop approval cards and the session menu describe ending only the agent, although a containerized Stop ends the
    whole container and its independent tab work. The agent-request route is shipped. Make both descriptions disclose
    that tabs end too.
  - The wrapper guide promises Stop preserves terminal tabs without naming the container exception. Qualify that
    sentence: a containerized Stop ends the whole container, including tabs.
  - Podman's default detach keys (Ctrl-P then Ctrl-Q) can leave the agent running while Farhelm shows it as exited;
    later Delete can kill it without the live-agent warning, and tab detachment can end the tab's shell. Disable detach
    keys for agent and tab connections; first verify the accepted spelling on supported Podman versions.
  - An optional token file inside the writable checkout can be replaced with a symlink to a readable host file; a later
    agent start or tab open then receives it as GH_TOKEN. This needs an unusual token-path choice. Refuse paths inside
    the checkout and read regular files without following links.
  - Podman inherits host proxy variables by default, potentially giving their credentials to the agent and tabs. This
    needs proxy settings in the host's login environment; network access is already unisolated. Decide whether to
    inherit them, and make the spec's environment list and code comment accurate either way.
  - Opening a tab in a stopped retained container can run old, agent-edited shell startup files against a replacement
    checkout. This needs a manual re-clone at the saved path while keeping the old session. Decide whether the accepted
    rule that tabs follow their working-directory path covers containerized tabs, and state it explicitly.
  - A create retry after a crash and reboot can remove an already initialized container if writing its launch spec
    fails, losing in-container files rather than the checkout. Several rare conditions must coincide; recovery before
    reboot was not ruled out. Decide whether that container counts as launch evidence, or explicitly accept the loss.
  - Account inspection of a checkpoint image may restore its old workload instead of inspecting an ordinary image. The
    route remains unverified in supported rootless use: restoration is root-only and requires a compatible
    checkpoint/runtime. Consider refusing checkpoint images and a root supervisor.

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
  Consider making ordinary sessions match. Plan: `plans/queue/stop-closes-tabs.md`.

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
  rather than reliant on remote round trips. Noted on 2026-11-07: it is unclear whether this is still a problem after
  the switch to new terminals (TBD).

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

- **Support adding remote macOS hosts.** V1 provisioning targets Linux hosts with a usable systemd user manager
  (SPEC.md, Topology); a Mac can only be added today by running the supervisor binary by hand, and the remote
  provisioning and remote Update flows do not cover it. Make adding a macOS machine as a remote host a supported flow.

- **Don't let git auto-gc run while a session is spawning.** On 2026-11-02, git auto-triggered gc on the farhelm repo
  itself when spawning a session — the user was actively waiting, which is exactly when auto-triggering is supposed to
  be deferred. Investigate what ran gc during the spawn (likely a jj/git operation in the colocated checkout) and move
  it to a moment when the user is not waiting. Plan: `plans/queue/no-gc-at-launch.md`.
- **Closing or starting terminals must not block other actions.** Closing a terminal or starting new ones should not
  disable doing other things, like switching to other sessions.

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

- **Paint a terminal snapshot straight into its final blob.** The snapshot encoder in
  `crates/farhelm-terminal-emulator/src/snapshot/` paints the escape-sequence stream into its own buffer and then copies
  it, with the fixups, into the blob it returns (`container` in `snapshot/v1/mod.rs`), so a snapshot's peak memory is
  about twice the blob, up to 32 MiB at the 16 MiB limit. Painting straight into the blob's buffer after a placeholder
  header, then patching the stream length (the header's last field), would bring that to about one blob plus fixups with
  byte-identical output. Left out of `faster-snapshot-encoder`, which is about speed; `tests/memory.rs` is the arbiter
  of the memory claim.
- **Built-in questions tool.** A built-in questions tool in the style of the interactive artifacts: how an agent asks
  the user a structured question (multiple choice, say), presented like those artifacts rather than as plain text.
- **Jittery/jiggling characters when switching between sessions.** Switching between sessions makes characters jitter or
  jiggle: something is being re-drawn or laid out for no reason. Find what triggers the needless re-draw/re-layout on
  session switch.
- **Per-host checkout directory.** Settings → Checkouts has one global **Checkout folder** for every host, but one
  directory rarely suits every machine: an absolute path from one machine may not exist or be writable on another, and
  hosts can differ in where projects belong. Let the user set the checkout directory per host in the UI, with the global
  value as the default. Today the only per-host override is `farhelm helm checkout-config set-root --host <id>`, which
  takes a numeric host ID that nothing shows the user.

- **Credentials for containerized sessions and ordinary hosts.** The Credentials section of Settings (planned in
  `plans/queue/settings-credentials.md`) serves only Tensorlake hosts, through their recipes. Extend it so containerized
  sessions and ordinary SSH hosts can use the same named credentials, for example a GitHub token in place of the
  containerized sessions' token file.

- **After tmux and xterm.js are dropped entirely: simplify terminal kind tracking.** While old terminals (tmux, drawn by
  xterm.js) and new terminals (terminal hosts, drawn by the grid) coexist, every layer tracks which kind each terminal
  is, and several pieces exist only so that a new terminal never touches xterm.js or tmux. Once a release stops
  supporting old terminals, every terminal is a host, and this tracking should collapse rather than linger as dead
  branches. The maintainer wants it removed then, not now. The pieces, as of the new-terminals cutover stack:
  - Page (`crates/farhelm-ui`): each terminal spec carries a kind (`host`, `tmux`, unknown, or `pending`), worked out in
    `session_view.rs` (`agent_spec_kind`) from the listing, the Restart reply's kind and whether a Restart is in flight.
    `pending` exists only so that the reconnect after a Restart that moves an old agent into a host never builds
    xterm.js; the reconnect controller in `assets/terminal.js` takes the latest synced spec for the same reason.
    `terminal.js` also keeps the xterm-first path for old and unknown kinds (`buildXtermSurface`, then `adoptGrid`
    swapping in the grid when the helm's stream notice says grid), the stream-kind chooser (`noteStreamKind`), and the
    old-terminal link adapter (`xtermLines`, `XtermFileLinksAddon` in `terminal-links.js` and `terminal-files.js`). With
    only hosts, the page can mount the grid unconditionally and drop the kind field, `pending`, the swap, and the
    xterm.js scripts themselves.
  - Protocol and helm: `TerminalKind`, `SessionInfo.agent_terminal`, `TabInfo.kind`, `StreamKind`, `Attach.grid` and the
    helm's `?grid=1` and stream notice. Keep a cached `"tmux"` kind decodable for as long as `helm.db` can hold rows
    from before the removal.
  - Supervisor: the `terminal_kind` column, the per-session tmux presence tracking (`tmux/presence.rs`), the refusals of
    Delete and Restart while an unusable private tmux server runs, `--new-terminals` and
    `SupervisorSeams::tmux_terminals()`.
  - Tests: the browser suite's old-terminal set (`FARHELM_E2E_SUITE_TERMINALS=tmux`, `e2e/suite-terminals.ts`, the
    `@old-terminal` tag) and the Rust tests that ask for old terminals by name (`harness_tmux`,
    `supervisor_process_tmux`, `tests::tmux_supervisor`).

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
  before the shape is settled. Related to, but not the same as, the grid stream encoding overhaul in this bucket, which
  keeps the split and changes how the same lines are encoded.

- **Review how the helm-to-supervisor connection multiplexes its streams.** One connection carries every terminal,
  upload, download, TCP relay and control reply as framed channels, and the maintainer wants to challenge its sizing and
  scheduling before more traffic (recipe tunnels) moves onto it. As read from the code on 2026-11-07, not measured:
  frames up to 8 MiB; uploads, downloads and relays in 256 KiB chunks with a 4 MiB credit window per stream, so one
  stream's throughput is capped near window / round-trip time; terminal output in 32 KiB chunks with no credit window
  (watermark pause and resume instead); on the helm, a single 64-slot writer queue with no priority lane, where bulk
  traffic is rationed to 4 queued frames ahead of keystrokes and acks; on the supervisor, priority, ordinary and bulk
  queues, with bulk drained only when the other two are empty, so sustained terminal output can starve relays and
  downloads; no cap on concurrent relays; no half-close on relays; and a 60-second write stall that ends the whole
  connection. Challenge the chunk sizes, window sizes, queue depths and the fairness between streams, measure where it
  matters, and change what does not hold up.

### Tensorlake

- **Agent-requested Tensorlake hosts.** Let an agent request a host by naming an existing recipe, behind the user's
  approval.

- **Agent subscription logins in Tensorlake sandboxes.** Support subscription logins and credential upload for agents in
  sandboxes, alongside the existing API endpoint settings. Plan: `plans/queue/settings-credentials.md`.

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

- **GH_TOKEN in Tensorlake sandboxes.** Inject `GH_TOKEN` into Tensorlake sandboxes like containerized sessions, from a
  named file on the host. Plan: `plans/queue/settings-credentials.md`.

- **Tunnels outliving a killed desktop helm on macOS.** Each recipe tunnel is an `ssh -N -R` child of the helm. An
  orderly quit stops them, and on Linux the kernel kills them when the helm dies (`PR_SET_PDEATHSIG`), but macOS has no
  parent-death signal: after a force quit, SIGKILL or crash of the desktop app, the tunnel ssh keeps running until its
  connection ends on its own, leaving the recipe's listeners open on the sandbox and possibly keeping the sandbox from
  idle-suspending. Make the tunnels die with the helm on macOS too, for example with a small watcher that notices the
  helm's exit (kqueue `NOTE_EXIT`) and ends them. Plan: `plans/queue/macos-tunnel-exit.md`.

- **Live resize of Tensorlake hosts.** A host is created at its recipe's size, and a recipe's size change affects only
  hosts created afterwards. Tensorlake can resize a running sandbox in place (CAS image format only): it takes one to
  two seconds and keeps processes running; CPUs go up and down, but memory never shrinks below what the sandbox booted
  with, so a sandbox meant to flex has to boot small. Let the user resize a Tensorlake host (or apply its recipe's
  current size) without recreating it. Phase 1 left this out deliberately.

- **Phase 2: a sandbox per session.** A session launch names a recipe instead of a host; Farhelm creates a sandbox for
  that session from the recipe's prepared snapshot, runs the session there, and deletes the sandbox with the session
  (one sandbox per session, mapped 1:1). Phase 1 (Tensorlake hosts from named recipes, prepared snapshots and Rebuild
  snapshot, suspend/resume, delete with snapshots, recipe tunnels, the list of sandboxes not on this helm) is the base;
  Rebuild snapshot was put into phase 1 because phase 2 needs exactly that path. Design records:
  `lore/2026-11-03-tensorlake-sandbox-hosts-design.md` ("Phase 2, as far as it was thought through", plus the measured
  platform facts) and `lore/2026-09-29-per-session-sandboxes-as-the-remote-host-goal.md`. What phase 2 needs that phase
  1 does not, as far as it was thought through:
  - Fast spawn. Streamed CAS snapshots start in about two seconds regardless of size (a clone runs about 2 s after
    create, a lingering user unit about 5 s later), so phase 2 depends on Tensorlake's CAS ssh gateway staying fixed (it
    broke and was fixed on 2026-11-03).
  - Per-repository snapshots on top of a recipe's prepared snapshot, refreshed with an incremental `git fetch` at spawn,
    and a refresh story for those snapshots. Fresh sandboxes have no Git credentials, so private repositories need
    credentials injected per sandbox; the GH_TOKEN entry above is a prerequisite, not a nicety.
  - Session-owned host rows that the hosts panel hides, with the session's lifecycle driving the sandbox's (create,
    suspend, delete), and cleanup that cannot leave billed sandboxes or snapshots behind when a helm dies mid-spawn.
  - Either the supervisor baked into the prepared snapshot with its identity reset per clone (clones also share
    `/etc/machine-id` unless it is emptied before the checkpoint), or a per-spawn install fast enough not to matter.
  - Answers to the trust questions in the 2026-09-29 entry: an unsupervised agent in a sandbox must not gain more reach
    into the user's other hosts than it would have anywhere else.
  - Possibly: live resize (boot small, grow on demand), shared scratch volumes, and forking a warm sandbox from a memory
    checkpoint or live copy (restores running processes with the same boot ID, which is also a trap: a forked supervisor
    keeps its identity).
- **`gh:owner/repo` launches can land in a session in the wrong repository.** When launching a GitHub managed checkout
  by typing `gh:owner/repo`, the string is sometimes treated as a session name instead, so the launch lands in a session
  in the wrong repo. Keeps happening to the maintainer. The fingerprint: the session's title is literally the typed
  `gh:owner/repo`, and it runs in a managed checkout of whatever was last launched (or the default). The maintainer
  types blind and only notices afterwards, so what the screen showed at the time is unknown.

  Findings from code inspection (2026-11-05, not reproduced): the New-session launcher (⌘N) does not appear able to do
  this. Its search parses `gh:` as the GitHub scope, and that scope never offers "Set session name"; a name row is only
  offered for unscoped text with no other matches. Enter there is guarded against stale rows and an empty box, and no
  timing race turned up. The quick switcher (⌘K) does offer "New session named …" for any unmatched text, but the
  maintainer is confident that is not the surface in use.

  Speculation: the text goes into the "Rename session" prompt that opens after a quick launch, not into the launcher.
  When "Name sessions on quick launch" is on (the default), any launch where the New-session form never appeared (⚡
  quick launch ⌘P, a numbered template ⌘1–⌘0, a ⌘X sequence, or a template launched from Templates ⌘J or the quick
  switcher) opens that prompt once the create succeeds, takes focus unconditionally, and selects the whole title. Typing
  `gh:owner/repo` and Enter there renames the session to exactly that string while it stays in the template's
  destination, which matches the fingerprint. For a managed checkout the create can take a moment, so the prompt may pop
  up mid-typing. This only holds if the key pressed was not actually ⌘N; the maintainer will watch the next occurrence
  to tell. Ways to confirm: look for the "Rename session" heading with the old name highlighted, or turn the setting off
  for a while and see whether the wrong-repo sessions stop.

  If confirmed, fix ideas: show the launched destination in the prompt and word it so it cannot pass for the launcher
  ("Name the session you just launched"); warn when the text looks like a launcher query (`gh:`, `tl:`, `host:`) that
  this only renames, and offer to open New session with it; possibly stop the prompt from stealing focus from typing
  that is already under way elsewhere.
- **A UI activity log that feedback can include.** Keep a timestamped log of what the UI did (dialogs, focus, shortcuts,
  requests to the helm and how long they took), and add an "Include application logs" checkbox to Send feedback. The
  point is general: whenever "I did X and something odd happened" or "I did X and it took about 5 seconds" comes up, the
  maintainer files feedback right after noticing, and the log says what actually happened and where the time went,
  instead of an agent guessing from code. The `gh:owner/repo` entry above is the motivating example: a log reading
  "shortcut ⌘P → quick launch; dialog opened: Rename session (after quick launch); focus → rename field; typed 11
  characters; Enter → submitted" would have settled in one report what code reading could only speculate about.

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

- **Deep dive on SQLite compaction settings.** Review how Farhelm's databases handle compaction: WAL checkpointing
  (nothing sets `wal_autocheckpoint`, so SQLite's default of 1000 pages applies, and the supervisor's database module
  says there is no checkpoint policy), auto-vacuum, and page size. Decide whether the long-lived stores need explicit
  settings or a periodic maintenance step as they grow.

## Doc todo

- Tell users that a working-copy (checkout) root must be a location only they can write: Farhelm is not designed for
  roots shared with or writable by other local accounts, and does not check. SPEC.md's fresh-checkout section states the
  rule.
- Tell users that closing, exiting, or deleting a terminal tab also stops services the tab shell's startup files started
  there first (an `ssh-agent`, an editor daemon, a detached tmux server), and that services the agent's launch started
  first are not affected. In an old terminal a tmux autostart guarded by `TMUX` never fires during the agent's launch,
  so when no tmux server of the user's is running yet, the first tab usually starts and owns it, and with `exec tmux` a
  detach ends the tab and the server; in a new terminal it fires during the agent's launch instead, and one that execs
  tmux keeps the agent from starting. Likewise, a personal tmux server or an ssh ControlMaster that the agent is the
  first to start ends with Stop, Restart, Replace or Delete, and one a command in a tab is the first to start ends when
  the tab closes or the session is deleted, each with everything the user later ran in it or through it, including from
  outside Farhelm. SPEC.md's session view section and "Ownership during cleanup and provisioning" state the rules.
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
  login, so shells an agent starts for its own work see no tmux either (a new terminal sets neither at any point, and
  tells programs it is Farhelm's own terminal with `TERM_PROGRAM=farhelm`). A startup file that starts tmux when `TMUX`
  is unset without first checking for an interactive shell (`[[ $- == *i* ]]`) or a terminal can then cut short the
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

- **A full browser report exceeded the recorder's inspection bound.** Browser sweep run
  `b0269440-1711-4470-8e18-f6f7ef88bec3` on clean main `2146e607ea9227579740704f2e120ee6192c2525` retained a
  33,090,209-byte `playwright.json`, above `scripts/test_run_playwright.py`'s 16 MiB report limit. Its metadata
  therefore has `runner.report.complete = false` and no validated aggregate counts, although the console reports 3,037
  passed, 18 skipped and three failed. The three isolated menu-test classification reports are complete; this is a
  tooling finding, not another flake. Decide how to retain validated full-suite counts while keeping inspection bounded,
  by reducing report payload or changing the inspection strategy. Raw report and console evidence remain in the retained
  run and expire with temporary-file cleanup; do not turn an over-limit report into a claimed validation pass.

- **Host menu fixture sometimes never opens before an expanded setup failure.**
  `expanded setup failure closes the menu below it` in `e2e/tests/provisioning.spec.ts` failed in Chromium browser sweep
  run `b0269440-1711-4470-8e18-f6f7ef88bec3`: the second host's menu flyout was absent through a five-second wait,
  before the assertion about closing it when failure details grow. All three isolated classification reruns passed on
  both engines (`1d205cfb-1e97-4479-96bf-d07b22e5dc3a`, `33e6b9ea-fbe3-49a5-a93b-acced1730ff3`,
  `919edd50-a83c-4c2a-a5f1-ee90f7387526`). Cause unknown; preserve the growth and menu-closing assertions (FLAKES.md,
  2026-11-05).

- **Visit-history deletion fixture stops in error instead of exited.**
  `visit history chooses and remembers the next session after row deletion` in `e2e/tests/sidebar.spec.ts` failed its
  stopped-session premise in Chromium sweep run `ceac5813-b025-478a-9019-a370b5fc97e5`: expected `exited`, observed
  `error` for five seconds. All three isolated classification reruns passed (`46ef9fa7-7605-4b68-9a5f-77a4e60d1e6c`,
  `7a1c431b-99c8-4581-b46b-b126c58b3407`, `63f93b25-ad0e-4db1-99a5-6b9367db2d30`). Cause unknown (FLAKES.md,
  2026-11-04). On 2026-11-05 it passed five attempts per engine across ordinary and host terminals; no fresh
  error-status detail was captured, and the exclusion remains.

- **Shallow pause resets under load.** `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay` in
  `crates/farhelm/tests/e2e/terminal_backpressure.rs` failed one of three exact nextest attempts on 2026-11-05 (batch
  `ef5851af-9d04-48d0-a378-9c256e2c10a7`, failure `dfecb8be-7208-44be-b9bf-047950e05e5c`). It rejects a reset anywhere
  in its ten-second post-resume drain, including a later legitimate tmux pause under load. The retained trace still
  evicts the needed attribution events; add a bounded per-channel cause observable before changing the assertion. The
  existing sweep exclusion remains. See FLAKES.md, 2026-10-17 and 2026-11-05.

- **Reconnect recovery misses its window under load.** `view-changes-do-not-postpone-a-recovery` in
  `e2e/tests/terminal-reconnect.spec.ts` passed five exact attempts per engine on 2026-11-05: three with ordinary
  terminals, two with host terminals. The historical 4185ms failure against 4000ms remains unresolved: driver and
  deferred terminal-mount latency are included in that observation. Keep the existing exclusion and timing assertion
  until a retry-deadline observable distinguishes rearming from late observation. See FLAKES.md, 2026-10-17 and
  2026-11-05.

- **Native tunnel preview exceeds WebKit's case budget.**
  `native tunnel preview keeps ports and actions inside a 650px pane` in `e2e/tests/header.spec.ts` timed out during
  cleanup at its unchanged 60-second deadline in recorder `1fc8687c-1044-4c63-83f9-dfddf8da2604`, then passed unchanged
  in 58 seconds in `9ebdc54c-8229-4787-a96b-19c2836e083e`. The new-terminal-button report records the same case timing
  out twice and passing unchanged in 58.7 seconds. Cause unknown; the narrow timing margin is a hypothesis, not a
  diagnosed fix. On 2026-11-05 it passed five attempts per engine; WebKit took 45.9–56.6 seconds across those runs. The
  deadline and exclusion remain unchanged; see the bounded-attempt follow-up in FLAKES.md.

- **Interrupted-card Replace clicks lost under load.** The WebKit test
  `don't ask again on a refused YOLO replace marks the host, then replaces with the override` in
  `e2e/tests/terminal-restart.spec.ts` failed in full browser run `49c74525-36ad-44e1-825b-878edadfd71c`: the Replace
  confirmation click landed but no YOLO question or replace request followed. On unmodified main `9f0fd7e9` it failed
  once in six runs under added load (`61cbd3cf-a2ab-4ebc-9b91-cc801ba932dd`) and passed four of four on a quiet machine.
  A Cancel click in the same card was lost the same way once in run `13808999-b14a-44ef-a7df-b547a8419a62`. Hypothesis:
  the spec's injected sessions use ids without the host-entry prefix #2368 made mandatory, so the UI's background
  session reads get 400s, and one completing at the click re-renders the card (FLAKES.md, 2026-10-20). On 2026-11-05 it
  passed five attempts per engine across ordinary and host terminals. The raw-ID hypothesis was not established by a
  fresh failure; the fixture and exclusion remain unchanged.

- **Same-source Replace menu fails to open after YOLO cancellation.**
  `plain Replace keeps a newer same-source key after old header YOLO cancellation` in `e2e/tests/replace.spec.ts` failed
  in WebKit waiting five seconds for the source row's menu in browser run `d01a19ad-811c-4dae-90b8-2c22a70ab473`.
  Isolated classification reruns failed, passed, failed: `213407df-5293-46eb-87ca-d08bbe6b3535`,
  `46df9503-ab98-4587-a36d-c8392c58183d`, `a4bb8764-8cf4-42f0-aeee-d97ad0267184`. A further WebKit repetition failed
  five of ten attempts (`0c25b7f9-40a5-47d2-af94-0c93edf7a331`). Trace and code review identify a product pointer race:
  dismissing the other row's open menu moves its error into that row, shifting the source toggle between pointer press
  and release. Leave the exclusion in place. An explicit test-side close would hide the bug; choosing between stable
  error space and changed dismissal/input ordering needs a product decision (FLAKES.md, 2026-10-17). On 2026-11-05 it
  passed five attempts per engine across ordinary and host terminals. The layout decision is still open; no test-side
  menu dismissal was added to hide the earlier product race.

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
  investigate a retained failure before treating interruptible startup commands as its fix. The complete cleanup script
  passed unchanged on 2026-11-05 in run `6f2eff90-c59c-4602-8283-489747b8a36d`, including spawner SIGKILL, spawner
  SIGTERM, script SIGTERM and both replacement-supervisor phases. This did not reproduce the leak.

- **Trash dialog's PageDown check on Chromium.** `trash header and global confirmation remain reachable` (both widths)
  in `e2e/tests/trash-dialog.spec.ts` sometimes leaves the Delete all confirmation's scroll area at the top after
  PageDown: sandbox runs `1a264a30-cd27-4f6e-bd2c-f3c3853fe0b6`, `e57f9fe2-36de-43f7-aade-46eadb89f6ee`,
  `89a737f6-3967-42aa-a519-a90238061970`, `e6e3b26c-8418-4e4a-9cfc-c3c976c4c8bf`; twenty runs each on main passed all
  (`7ce0fa35-3abc-400d-ab8d-5fc457bb0731`), with launch-progress failed two (`d30273b9-fc29-4e19-b4c0-036465123b34`) and
  three (`03eda759-efdb-41ff-956b-564f134ad83d`), and it also failed on a stack without launch-progress. Cause unknown;
  whether launch-progress's waiting status raises its rate is open (FLAKES.md, 2026-11-04). Both widths passed five
  attempts per engine on 2026-11-05 across ordinary and host terminals. Focus/scroll assertions and exclusions remain
  unchanged; no fresh trace establishes why the earlier PageDown had no effect.

- **The launcher's YOLO question does not take its answer on WebKit.** `answerYolo` in `e2e/tests/helpers/term.ts`
  (through `fillCreateForm`) failed with "Clicking the checkbox did not change its state" in three tests in three
  rounds: two in `e2e/tests/terminal-multihost.spec.ts` and one in `e2e/tests/terminal-create-idempotency.spec.ts` (runs
  `a865423e-2157-473d-bd78-5bafcce7811e`, `7033e514-e01f-435e-993c-57b6e4ba5126`,
  `55c4bee4-892d-4118-968c-d37cca4c920f`); each passed three re-runs. The step is shared, so other tests can fail the
  same way. Hypothesis: the click lands while the launcher re-renders the question (FLAKES.md, 2026-11-04). Each of the
  three excluded callers passed five attempts per engine on 2026-11-05 across ordinary and host terminals. No click
  retry or answer-helper change was made without evidence of the missed transition.

- **An update's progress label shows no step name while following.** In `e2e/tests/provisioning.spec.ts`, three "while
  following" tests failed waiting for `Updating: N/M <step> m:ss`, the label holding only the count and timer for five
  seconds (runs `8add8072-2ed9-4899-a9ac-d1f5bf3c95d3`, `4d9291ef-904c-4048-ae81-b527a60f6449`,
  `541a7a0e-0619-4e57-b9ba-2d345c1279f8`); each passed three re-runs. Hypothesis: the step name arrives late under load
  (FLAKES.md, 2026-11-05). Each of the three cases passed eight attempts per engine on 2026-11-05: the three-attempt
  baseline plus five more exact repetitions. The controlled feed can retain an initial all-pending snapshot, but no
  fresh failure established that as the cause. Step-name assertions, time budgets and exclusions remain unchanged. A
  fourth test, "incarnation churn keeps a followed run while a target change detaches it", failed the same way on WebKit
  in run `269f1dd3-f429-4b1d-9171-523f76ef94d9` and passed three re-runs (FLAKES.md, 2026-11-06).

- **Quick launch's focus does not return to the page body on Chromium.** Two tests in `e2e/tests/quick-launch.spec.ts`
  poll for the body to hold focus after a blur and sometimes never see it (runs `2d338903-2d70-4c8c-99c4-3d4044d78670`,
  `ffe0693e-0a67-4039-a8e4-4621365c4561`, `4b6e1aad-91ab-4aee-b721-f760fb42b0cf`,
  `443ebd09-f5c5-4cd3-8ad1-d27e96a75532`). Hypothesis: a focus handoff in the page takes focus back (FLAKES.md,
  2026-11-05). Both cases passed five attempts per engine on 2026-11-05 across ordinary and host terminals. A delayed
  focus return remains a hypothesis; neither the body-focus premise nor the exclusion was removed. The
  typed-page-refusals case recurred on Chromium in the host-terminal run `b8e18c76-972f-4e97-bf66-828778581c59` at
  `0563c44f`; WebKit passed, and the failure snapshot still had terminal input focused after the blur. No additional
  reproduction or fix was attempted (FLAKES.md, 2026-11-05).

- **Settings save cleanup waits for a request-finished event WebKit never sends.** Two checkout-save tests in
  `e2e/tests/settings.spec.ts` (post-clone commands with carriage returns, and a pending checkout folder save surviving
  close) time out in cleanup on `reply.finished()` (runs `40e6f124-6214-4bd7-865c-64740aecd10e`,
  `b9bc59c5-c98f-4e0d-92fc-6735b2c9fc50`); host-terminal re-runs `cb57caa4-7c10-4a8b-9a8f-157331af8daa` passed.
  Hypothesis: the WebKit behavior #3061 fixed in two sibling tests with a drained route (FLAKES.md, 2026-11-06).

### Difficult deflake

- Restore the release integration gate after the remaining Rust flake reports are resolved. The binary-output loss and
  degenerate-size READY matcher have fixes recorded in FLAKES.md (2026-09-30 and 2026-09-07). The older stalled-viewer
  RSS and malformed-sentinel reports each passed three focused nextest attempts on 2026-11-05 in batch
  `51c6a652-7437-4d1a-a579-920a936cc00b`; those non-reproductions do not close them. The shallow-pause reset still
  recurs, as its Deflake entry records. Keep the integration target's exclusion in `.github/dist-build-setup.yml` until
  evidence supports reversing it. Browser flakes are separate coverage and do not themselves gate that Rust target.

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

- **Pluggable host creation.** Let users plug in their own scripts that spawn and destroy hosts for sessions to run on,
  rather than Farhelm knowing each provider itself. Related to Tensorlake hosts and the Sprites host-kind entry above,
  which build specific providers in.

- Sandboxed agents with scoped GitLab credentials, especially on Tensorlake hosts and the proposed Fly.io Sprites hosts:
  give an agent a token that reaches only the repositories it works on, minted per sandbox. GitLab rather than GitHub
  because GitLab lets the token creation be automated, while GitHub's scoped tokens have to be created by hand in a
  browser.

- Extend containerized sessions to the macOS desktop, help users prepare or obtain images, and consider stronger
  isolation beyond the experimental Linux rootless Podman support. Plan: `plans/queue/podman-sandbox.md`.

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
  as untrusted, go through the grid's `textContent` spans (xterm.js cells for old terminals), Dioxus's escaped text or
  `textContent`, never `innerHTML` or `dangerous_inner_html`, and Rust-built page script interpolates only
  JSON-serialized values. Exploiting the fallback would need a new script-injection bug in Farhelm's rendering or
  xterm.js, or a navigation escape in Dioxus or WebKit. Such script would already hold the device secret and could drive
  the whole helm API, typing into terminals included, so the file read adds a quieter route to files rather than new
  power. It is wanted at all because SPEC.md's "Client hardening" holds the native app to a higher bar: proportionate
  hardening that narrows what a hypothetical flaw could reach.

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

- **Check upstream xterm.js for two link bugs at the next upgrade.** After part of a wrapped URL is repainted, clicking
  it can still open the old target, and on very long wrapped lines a URL's click area shifts onto the row above. Farhelm
  does not work around non-security bugs in the vendored bundle (SPEC_impl.md, decided 2026-10-10), so when the bundle
  is next upgraded, check whether upstream fixed these and remove this entry.

- **Investigate harness-native notifications.** Claude Code and Codex can raise desktop notifications themselves, by
  writing terminal escape codes that a supporting terminal turns into a system notification. Claude Code picks the
  channel with its `preferredNotifChannel` setting (in `/config`; values such as `iterm2`, `ghostty`, `kitty` or
  `terminal_bell`) and fires when it needs the user: a permission prompt, or sitting idle waiting for input. Codex does
  it with `notifications = true` under `[tui]` in `~/.codex/config.toml`, and fires when a turn finishes and on approval
  requests. Inside Farhelm those codes reach the supervisor's own terminal emulator in a new terminal, which does
  nothing with them, and tmux in an old one, which drops them unless `allow-passthrough` is on (Farhelm does not set
  it), and past tmux they would land in xterm.js. Either way they end in Farhelm's terminal rather than a native
  terminal app. Investigate what Farhelm could do with them: surface them as client notifications, use them as a status
  signal, or nothing. Related to the ntfy.sh entry under Near term.

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
