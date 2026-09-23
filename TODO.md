# TODO

A running list of things the maintainer wants fixed or built. This is intent, not history: an entry is REMOVED in the
same PR that addresses it, so the file only ever describes what is still wanted. It is not a roadmap and carries no
priorities unless an entry says so itself.

Ten buckets, assigned by the maintainer: "definite simplification" is complexity the maintainer has decided to remove —
the decision is made, only the work remains; "planned" holds accepted work to implement later and suppresses duplicate
review triage within each item's stated scope; "near term" is what should be picked up next; "doc todo" holds
documentation work; "tricky bugs" retains unresolved bug reports and their investigation findings; "deflake" gathers test
and harness reliability work, including CI execution and restoring gates; "broken tests" records tests that fail
deterministically, with the failure and the evidence that it predates any in-flight work; "code review" is the residue
of the September 2026 review swarms after the policy pass, ordered by confidence and risk; "maybe later" is wanted but
not soon, and may never happen; "unbucketized" is everything not yet sorted, which carries no implication either way.
Within a bucket, no order unless the bucket explicitly says so.

Known product fixes stay in their product bucket. "Difficult deflake" retains unresolved failures and their
investigation evidence within "Deflake"; that placement does not establish that the cause is test-only. Move a diagnosed
product fix out of "Deflake" rather than changing user-visible behavior as a test correction.

## Definite simplification

## Planned

- **Keep session creation off the connection read loop.** Run creation through tracked background handlers so ordinary
  launch work and admission waits do not delay terminal input or unrelated requests on the helm's shared connection to
  that supervisor. Cover both full-authority and session-authenticated create paths; preserve bounded handler admission,
  credential revalidation after waits, keyed-create durability, and disconnect cleanup. Verify with a deterministic
  regression that another request or terminal input progresses while creation is paused. Medium effort: localized
  dispatch changes, with care around task ownership and existing lifecycle guards. This plans the fix described in
  `create-runs-inline-on-read-loop.md`; it does not request immediate implementation.

## Near term
- Remember Farhelm's expanded/full-screen state across restarts so users do not have to double-click to expand it each time. Use the platform's standard restorable-window-state behavior, validating saved bounds against the currently connected displays and falling back safely when the display layout changes so the window never reopens off-screen or unusably large. The current native API boundary is recorded in [docs/window-restoration-investigation.md](docs/window-restoration-investigation.md).
- Fix the vertical alignment of the destination selector and browse-folders button in the new-session form; their labels should be centered within their controls.
- Surface an `old version` status for hosts even when the supervisor and helm have no protocol version skew; reserve `needs update` for the incompatible-supervisor case.
- Add an `update all` action that updates all remote hosts in one operation; details are TBD.
- Modernize the interrupted-session view shown after the host running the supervisor or agents restarts. Make clear that the session must be restarted intentionally, then offer clean, Farhelm-styled `Restart` and `Replace` buttons instead of making the state look like a failure.
- Audit the remaining GUI buttons for controls that are rendered as plain gray text or otherwise do not match Farhelm's visual style, and make button styling consistent throughout the GUI.

- Remember Farhelm's expanded/full-screen state across restarts so users do not have to double-click to expand it each
  time. Use the platform's standard restorable-window-state behavior, validating saved bounds against the currently
  connected displays and falling back safely when the display layout changes so the window never reopens off-screen or
  unusably large. The current native API boundary is recorded in
  [docs/window-restoration-investigation.md](docs/window-restoration-investigation.md).
- **Claude foreground ownership.** Assess whether native or shelled-out Claude children can replace or withdraw the
  foreground conversation's restart target, then define the smallest admission check that preserves legitimate
  clear/new/switch/fork/resume transitions. This is an assessment task, not a claim that every vendor path has been
  reproduced.
- **Pi foreground ownership.** Assess whether native or shelled-out Pi children can replace or withdraw the foreground
  conversation's restart target, then define the smallest admission check that preserves legitimate foreground
  transitions. This is an assessment task, not a claim that every vendor path has been reproduced.
- **OMP foreground ownership.** Assess whether native or shelled-out OMP children can replace or withdraw the
  foreground conversation's restart target beyond the checks now landed in #814, and define any remaining smallest
  admission check. Preserve legitimate foreground transitions and avoid extending the reporter's scope without
  evidence.

The earlier cross-harness evidence is preserved in [the historical ownership
assessment](lore/2026-09-20-harness-conversation-ownership.md).

## Doc todo

- Bring the README overview/splash content into the main documentation.
- Document the harness support feature matrix so supported and unsupported features are clear.
- Answer "Is it vibe coded?" with a clear explanation.

## Tricky bugs

- Investigate corruption in the Codex input area when typing quickly. In ordinary use, appending exactly
  `include a SPEC.md` to a prompt quickly made the display show `include a SPE` followed by another line containing
  scattered fragments such as `COMMI`, `PR`, and repeated `SPEC` text, with large gaps between them, before submission.
  No bug screenshot or logs were supplied. Whether the underlying input was corrupted or only its rendering is unknown.
  [Investigation findings](docs/codex-input-investigation.md): direct tmux and Linux Chromium/WebKit probes did not
  reproduce the scattered current input; the report remains unresolved, including native macOS coverage.

- Investigate Codex resuming the existing conversation after using "Replace" on a session. Reported in ordinary use: the
  replacement retained the previous conversation and could summarize the earlier work, instead of starting a fresh
  conversation. Replace should preserve the session's directory, title, and agent choice while starting fresh. Cause and
  reproducibility are not established. [Investigation findings](docs/codex-replace-investigation.md): controlled and
  real Codex comparisons started distinct replacement conversations, including after Resume; the incident remains
  unresolved without its launch configuration and vendor conversation identities.

## Deflake

- Watch the island-cap readiness residual in `e2e/tests/terminal-tabs.spec.ts`:
  `a tab list past the island cap is listed in full but only partly attached`. Its original first-mount shape — a
  handshake that stalled, was bannered and closed at 5s, and never retried — was fixed by the maintainer's 2026-09-17
  decision to retry never-connected first mounts on the ladder (`crates/farhelm-ui/assets/terminal.js`; evidence trail:
  `lore/2026-09-16-island-cap-never-connected-first-mount.md`), and that fix is visible in post-fix failures: the agent
  socket now OPENS where the recorded shape had it closed at readiness. What remains surfaced on 2026-09-17 on a loaded
  machine (another agent's build running): 2 of about 6 WebKit executions failed with `open=true, revealed=false` for
  the full 20s readiness budget — the handshake completes but the attach/replay behind it starves across ladder
  attempts, each hidden mount cycling the 5s watchdog until the budget expires; the retained trace was then wiped by
  later runs, so only the readiness observation is recorded. That is a deeper layer of the same burst pathology a retry
  cannot fix by design (a stall persisting across attempts), not a regression from the retry change; the fork to settle
  on recurrence: instrument the supervisor's attach path under the churn burst, extend the readiness budget against the
  retry cycle, or accept the residual. Keep this distinct from the existing single-client stall entry, and do not weaken
  liveness assertions based on a later passing run.

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

### Systematic deflake

Deferred work, with its original triggers:

- A typed scale factor on harness budgets, triggered by a budget-class recurrence after the readiness-oracle changes
  land. Keep harness budgets distinct from product deadlines (`Budget::harness` versus `Budget::product`, and
  `expect.configure` plus a browser helper); `terminal-flood.spec.ts` has both kinds of deadline. A slow wait caused by
  an invalid fixture premise is not evidence for scaling its budget.
- Product observables that attribute their cause without changing the deliberately shared stall-detach reason string:
  supervisor stall versus helm backstop, sentinel unlink path and errno, and bounded helm-death detection latency.
  Attribution belongs in a secondary field or retained log line. This remains product work for "Near term" when the
  maintainer chooses to schedule it.
- Running the tag gate's suite away from the release build, after "Restore the release integration gate" lands. The
  release gate still excludes the e2e target; evaluate any concurrency experiment using the then-current runner budget
  rather than reviving the old libtest thread setting.

## Broken tests

## Code review

The residue of the September 2026 review swarms (700 findings over seven areas, reviewed at `db76f00b`) after the policy
pass applied the maintainer-confirmed decisions in SPEC.md, and after a per-finding check against main at `7c2cdd83` on
2026-09-13 established which mechanisms still exist. Finding IDs such as `A5-C4` are area-N plus the finding's tag in
that area's review; each resolves to a line in the brain's verbatim mirror, linked from
[this page](https://claude.ai/code/artifact/5a79de5b-90d5-4eee-8aa1-ea3c803ef50c). The full working archive, including
the fourteen group notes whose fences the entries below quote, is in the private brain under
[farhelm-review-postprocessing](https://github.com/scode/brain/blob/main/personal/farhelm-review-postprocessing.md); the
raw swarm reports are the seven mirrors beside it. The final synthesis is also on
[HackMD](https://hackmd.io/a2F0fZKqQsSh4anyxFrjlA?type=view).

Every entry names the code it rests on as of the check; a later reader must confirm the mechanism is still present
before building. "Fence" is what the group notes say the fix must preserve or must not become. Severity and effort are
agent judgments, not measurements. Nothing here authorizes a broader rewrite than the entry names: the notes repeatedly
warn against folding neighbours into one lifecycle or locking overhaul.

Closed by the check, so nobody re-raises them: `A4-T8` was fixed on main by the per-test capture buffers (#419, #423);
`A3-C8` (delete cancels uploads before its preflight) is accepted by SPEC's partial-deletion decision; `A6-C6` (the
14→15 profile-table drop) and `A6-C7` (per-session SQL parameters at fleet scale) are excluded by the compatibility and
client-scale decisions.

### Do first: high confidence, low risk

Mechanism verified on main, fix is trivial or small, and the notes attach no design question. Ordered roughly by
severity.

- **Snapshot self-witness.** `A3-C3`. High, trivial. `procs::snapshot` (procs.rs:372-402) returns `Ok` with an empty map
  when `/proc` is present but unmounted, and the macOS path (procs.rs:765) returns `Ok(Vec::new())` for a zero-sized
  `KERN_PROC_ALL`, so every stop and delete reports success having examined nothing, against the module's own
  fail-closed contract at procs.rs:91-99. Fix: after the walk, return `Err` unless the map contains
  `std::process::id()`. Also backstops the real-uid changes below.

### Next: high confidence, needs care

Mechanism verified on main, but the fix touches lifecycle, locking, or the kill set, or needs a reproduction before it
is safe. Each is its own review unit.

- **Real uid in the process walk.** `A3-C2` then `A3-C1`. High, small on macOS and medium on Linux, medium risk because
  widening the table widens the kill set. macOS `snapshot` (procs.rs:826) filters `kinfo_proc` rows by `cr_uid`, the
  effective uid, while `p_ruid` sits transcribed at :599 marked "never read"; Linux (procs.rs:391, :343) uses
  `/proc/<pid>` directory ownership, which also tracks the effective uid. A descendant that execs a setuid binary, and
  everything below it, is silently dropped, and stop reports success; macOS has no cgroup backstop. Fix: on macOS add an
  offset assertion for `kp_eproc.e_pcred.p_ruid` beside the four at :678 and accept a row when real or effective uid
  matches; on Linux select by the real uid from `/proc/<pid>/status` and keep the directory-owner check only where
  `read_process` uses it to classify a failed read; correct the `snapshot` docstring's "not killable" premise. Do macOS
  first, Linux second, each with the self-witness above already landed. Fence: ordinary descendants in scope, deliberate
  same-account escape not; never broaden a kill set on identity that has not been revalidated.
- **Tab close leaves input aimed at the agent pane.** `A5-S4`. High, small, medium risk. `close_tab_window`
  (core.rs:8901-8932) reaps, kills the window, reaps again, and only then calls `detach_closed_tab`; the audited
  `=<session>:.<pane>` target doc (tmux.rs:1552-1559) records that a vanished pane silently degrades to the session's
  active pane, which is the agent window. Keystrokes typed in the seconds between kill and detach can land in the
  agent's pane. Fix: move `detach_closed_tab` ahead of the reap and kill, or invalidate the attachment's `InputClient`
  under the attachments lock immediately before the kill. Fence: the fallback was audited for `display-message`, not
  `send-keys`; verify input delivery separately from output capture. A focused real-tmux reproduction on pinned tmux
  3.7c instead got `tmux send-keys input command failed: can't find pane: %1`; the next attempt should start by checking
  whether the production connection path changes that control-client reply.

### Later: low confidence or needs an argument first

Real enough to keep, not established enough to act on. Each names what would settle it.

- **Capture parsing at the 64 KiB prefix.** `A5-C20`. `read_prefix` (agent_kind/capture.rs:626-630) takes a flat byte
  cut, and one unparseable leading line marks the whole scan incomplete and blocks every durable claim. Conditional on a
  supported vendor record whose first line exceeds 64 KiB and on no successful identity hook; neither verified. If
  shown: make the front of the read line-aware under a hard ceiling, or report mid-line truncation.
- **Partially removed `Farhelm.app`.** `A2-C25`. install.sh:1084-1090 requires `Contents/Info.plist` and :1140 does
  `rm -rf` then a cross-volume `mv` from the staging directory, so a partial failure leaves a plist-less directory the
  guard reads as foreign. Fix would stage into a same-filesystem sibling and move the old bundle aside; a missing
  Contents directory is not proof of ownership, and a foreign collision must never be deleted.
- **One-sided activity clock guard.** `A6-C22`. `record_activity` (supervisor store.rs:3660-3665) accepts only forward
  moves, so a single forward clock jump pins the stamp. The notes want the whole activity, seen, and merge path
  investigated before any clock slack, and do not accept the literal permanent-freeze claim for every excursion.
- **Descriptor ceiling and terminal socket admission.** `A4-C8`. `render_helm_unit` sets no `LimitNOFILE` or
  `StartLimit` directive, `serve_term_upgrade` takes no seat where `events_ws` does, and with the fatal accept path
  above the chain ends with the helm down and not restarting. Reachability in ordinary use is the open premise, and
  SPEC's handful-of-clients decision argues against treating hundreds of terminal sockets as expected. If pursued:
  `LimitNOFILE` first, non-fatal accept second, admission control last.
- **Aggregate input cost under the attachments lock.** `A7-C33`, `A5-C9`. `InputClient::send` has no total bound and the
  caller holds the supervisor-wide lock across it; the shipped helm chunks at 32 KiB so ordinary use is about 128
  pipelined round trips, and SPEC's local-authority decision removes the hostile framing. State the aggregate cost in
  the contract; if a bound is wanted, cap what one frame may carry where the chunking lives rather than releasing the
  lock mid-send.
- **Relaxed ordering on the relay's issued-id check.** `A1-C12`. agent_relay.rs:270 issues ids Relaxed and :374 compares
  Relaxed before the pending-table lock, then retires the connection on a miss. Nothing in-process establishes a
  happens-before, but the proposed Acquire/Release swap does not either, since the reader never synchronizes with the
  issuer. Either document the real ordering argument or make the check consult the pending table.
- **Capture columns after a failed non-Resume restart.** `A6-C5`. `begin_relaunch` clears five capture columns and
  `PriorRun` restores four other fields; the headline loss of a usable conversation is unreachable because the mode is
  validated against a non-Resume offer, leaving unrestored `first_input_at` and `capture_ambiguous` plus overbroad
  restore prose. Confirm a reachable consequence first; otherwise correct the prose.
- **Local install path without fsync.** `A2-C19`. backend.rs:620-644 renames a payload into place after `flush()` with
  no `sync_all`; the panel refuses local provisioning outright, so production reachability is doubtful. Trivial if ever
  wanted; no blanket power-loss guarantee and no remote-branch change.

## Maybe later

- **Goose foreground ownership.** Keep the basic Goose reporter for now; it does not distinguish a native or shelled-out
  child that inherits the reporter from the foreground conversation. The stricter database-backed implementation is
  preserved at the `goose-capture-complex-2026-09-23` tag for comparison. The research and proposed smaller replacement
  are recorded in [the Goose session-tracking assessment](lore/2026-09-23-goose-session-tracking.md). Revisit only if
  reliable child isolation becomes a product requirement or Goose exposes a direct root/subagent role signal.

- Reconsider the first-use configuration experience for `gh:` launches when no working-copy root is configured. The
  first version refuses the launch and points to the CLI command; consider an inline GUI flow on initial use or another
  improvement that makes setup easier. Keep the general preference for CLI configuration of rarely changed settings.

- Coordinate uninstall with installation, setup, and runtime startup. Share the relevant locks and revalidate removal
  targets under them so an update, setup, desktop launch, or new session cannot race uninstall's checks and deletion.
  Deferred from initial standalone uninstall support; that first version assumes these operations do not run
  concurrently.

- Extend Muse beyond basic terminal launching: integrate per-launch hooks/instructions, capture the correct conversation
  identity for resume, and recognize Muse's waiting/status signals. Built-in `muse` and `muse-yolo` profiles currently
  use generic activity status without hooks or conversation resume; these are Farhelm integration gaps, not established
  limitations of Muse.

- Make `install.sh`'s output easier to scan. The completion message is a wall of text mixing installation results,
  restart instructions, and setup advice. Improve the layout and visual hierarchy, possibly with color; details TBD.

- Free up more space for session names in the sidebar: the agent and yolo/profile labels currently take too much of each
  row. Use one small indicator for the agent and a separate small indicator for whether it is running in yolo mode.
  Decide the agent indicator's form (SVG icon, short name, etc.) and the remaining display details when doing the work.

- Separate an agent profile's common invocation (how to invoke the agent), initial launch arguments, and resume
  arguments into three independently specified parts. The immediate restart fix is deliberately simpler: when no
  explicit resume template is supplied, reuse the original invocation and append the agent's resume syntax, assuming
  every original argument is reusable and there is no initial prompt, launch-only input, or custom command shape to
  interpret. Keep the immediate fix to launch arguments plus resume arguments; custom argument handling belongs to this
  follow-up. It must remove that assumption: initial prompts and launch-only options must not be replayed on resume, and
  existing resume/continue selectors or an end-of-options marker must not collide with the generated resume command.
  Preserve shared permission and configuration options without requiring users to duplicate them between launch and
  resume. Settle the composition, editor, and migration behavior when this is picked up; keep it out of the immediate
  fix.

- Reconsider agent parent/child relationships: either remove them or make them useful. Current parent tracking is
  optional, and fleet `agent create`/`clone` do not record the asking session, so the parent filter cannot reliably
  answer which sessions an agent created. This is largely unused complexity today; assess whether useful tracking is
  worth keeping before extending it. The current limitation is explicitly accepted in SPEC.md.

- Close the cross-host execution hole in agent-requested session creation and cloning, then remove their temporary
  exception from the host-isolation policy. These operations currently let a remote host cause arbitrary execution on
  another host; this is explicitly accepted temporarily to defer redesign, not permission to add more such operations.
  Preserve the eventual ability for agents to orchestrate sessions across hosts through an explicitly authorized launch
  policy, potentially trusted profiles, without letting the requesting host choose arbitrary execution. Include the
  existing agent/supervisor-originated creation and retry paths: a delayed resubmission must be considered when deciding
  what launch authority remains valid, including whether a forgotten retry key can launch a session again. Permanent
  retention of these agent-originated retry records is not required; their replay exposure is accepted pending this
  work. Do not add further exceptions or infer a waiver of user-initiated GUI request correctness. Cross-host stop and
  rename remain intentionally allowed bounded operations.

- Let the user mark each host as "yolo is fine" or "yolo is not fine", controlling which hosts appear red in the session
  list. This could also support warnings when the user is about to run an unsandboxed agent on a host marked "yolo is
  not fine".

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

- Automate end-to-end testing of the host UPDATE path, including across releases. Nothing in CI updates a host: the
  CentOS provisioning test only ever installs onto a fresh container, and the update flow's own tests drive fake
  backends. The gap shipped a real field failure (2026-09-01), which is the worked example any design here should be
  checked against: the first cross-protocol update ever attempted — a protocol-12 farhelm 0.1.1 host under a protocol-14
  0.2.1 helm — failed at the PROBE, whose classifier treated the version-skew refusal as a transport failure ("the
  supervisor probe closed before hello completion with exit status 0"), making exactly the host the update action exists
  for un-updatable; the operator recovered by stopping the remote supervisor by hand so the probe would see clean
  absence and take the fresh-install path. The eventual fix (`ProbeObservation::SkewedSupervisor`) added unit and
  service-level regression tests, but the CLASS of bug wants end-to-end coverage: something like a CentOS-leg variant
  that provisions a PREVIOUS RELEASE's binary (the harness builds its payloads from this tree today; the helm's own
  verified release-download path — D13, `release_payloads.rs` — is the existing machinery that can fetch a pinned
  released one), lets it register and run, then drives the panel's update action to the workspace build and asserts the
  supervisor comes back at the new version with its tmux sessions intact. The old half must be a real released artifact,
  not this tree's build — same-version update tests are exactly what could never see this bug.

- Guard provisioning against pushing payloads older than the helm's own protocol. A release-shaped helm built from a
  commit newer than the latest release (the local stable-binary flow does exactly this) provisions remote hosts with
  DOWNLOADED released payloads by default (D13), so the freshly provisioned supervisor can speak an older protocol than
  the helm that just installed it — and the helm then refuses it at the hello gate. Nothing is damaged (the refusal is
  the version rule working), but the failure arrives one step late, as a skewed host instead of a refused provisioning
  attempt. Possible shapes: compare the payload's version against the helm's `PROTOCOL_VERSION` before pushing and
  refuse with a message naming the mismatch; or make the staged-payload path (`--payload-dir`,
  `FARHELM_HELM_PAYLOAD_DIR`) the documented answer for from-main helms. Noted 2026-08-31 when upgrading the stable
  install to a from-main build while the newest release was still 0.1.1.

- Custom hover tooltips on buttons and menu items. Native `title` tooltips are free (the UI already uses them on the
  activity time, the cwd line and the profile chip) but the browser owns their ~1s delay and nothing — no CSS,
  attribute, or JS — shortens it; WebKit's web content ignores the macOS tooltip-delay default too. A faster, themed
  tooltip is a component shown on hover after a delay of the app's own choosing (~300ms), and it has to escape the
  sidebar: `.app-sidebar`'s `overflow: hidden auto` clips anything anchored inside a row near its edges, so the tooltip
  needs a body-level portal or `position: fixed` with measured coordinates — the row `…` menu's popover is the pattern
  to copy. If the native delay turns out tolerable, a `title` pass over the terse actions (stop / delete, the host row's
  buttons) is an hour and needs none of this.

- Consider dropping conversation-identity SCAN support and keeping only the per-launch hook. The resume promise stays;
  what goes is the second mechanism. The hook is the agent's own answer and covers `/clear` and `/new`, which the scan
  cannot see at all; the scan (`agent_kind/capture.rs`, `service/capture.rs`, and their e2e suites — roughly 6k lines)
  exists only for launches where the hook cannot be attached (a profile already passing `--settings` or Codex hook
  config, a bare `--`, or `FARHELM_AGENT_HOOKS` opting out) and for a hook that failed. Those launches would take the
  fallback SPEC.md already defines for an uncaptured identity — restart says so and offers the resume template or a
  fresh launch — and the vendor-record parsing that breaks whenever a vendor changes its on-disk layout goes away. The
  spec edit is one clause in Durability and resume ("and scanned from the outside … otherwise", plus "Scanning stays the
  fallback …"). Write-up: https://claude.ai/code/artifact/554790ce-c744-4daa-b9a5-151facdb1f42

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

- Replace xterm.js with libghostty via WebAssembly, assessed 2026-09-12 in `lore/2026-09-12-libghostty-assessment.md`.
  Native libghostty on macOS is ruled out there: it owns its own PTY and renders into an NSView, neither of which fits a
  WebSocket-fed terminal inside a webview. The WASM route through `coder/ghostty-web` (xterm.js-compatible API over the
  upstream wasm) is plausible but not a drop-in: `onBinary`, `parser.registerOscHandler`, `buffer.active`, and `refresh`
  are missing, its `write` parses synchronously so the backpressure model in SPEC_impl.md has to be re-derived, and the
  upstream wasm ships only on the rolling `tip` release with nothing stable to pin. Medium to high effort; the payoff is
  dropping the scroll-freeze workaround and getting Ghostty's own grapheme and SGR handling. First step is a one- or
  two-day spike mounting ghostty-web in the island under WebKit.

## Unbucketized

- Make the never-started verdict say which link died. When a scoped launch dies before farhelm's exec shim, the
  supervisor's `wrapper_failure_detail` (launch_artifacts.rs) records "the agent was never started: the launch never
  reached farhelm's exec shim, so something before it — the transient cgroup scope wrapper, or the login shell itself —
  exited first", which names two suspects and separates neither. The wrapper's stderr is still sitting in the dead pane
  under `remain-on-exit`, so a `capture-pane` at classification time could say which. A first attempt (2026-08-23)
  appended the pane's last words to the durable `LastOutcome::Error` detail and was withdrawn in review for three
  reasons any retry must design around: (1) SPEC.md's terminal-retention contract — terminal content lives only as long
  as the host-side terminal, with no separate history store — which a durable excerpt of startup/rc output contradicts,
  so either the quote must not be persisted (log it, or surface it only while the pane exists) or the spec must
  authorize a bounded exception first; (2) the pane is reused across relaunches and keeps its scrollback, so a
  generation N+1 that died before printing anything would quote generation N's conversation unless the capture is fenced
  to text written after the wrapper started; (3) the ownership-and-deadness check and the capture are separate steps
  with no lifecycle claim across them on the list path, so a same-pane restart in between would quote a live later
  generation — revalidate atomically with the capture, and budget the capture so N never-started rows cannot cost N tmux
  timeouts on the hot list. The e2e harness already has `wait_for_agent_ready` (harness.rs), whose failure text shows
  the same pane text for a test's own diagnosis; that is the non-durable shape to start from.

- Decide the reservation tombstone scope for interactive creates, then do the work the verdict leaves standing. When a
  client attaches an intent key to a create, the supervisor records it in `create_reservations` so a retried request
  returns the already-created session instead of double-launching an agent. The durability-era decision made these
  reservations PERMANENT for interactive creates (spawn's are session-bounded), which makes `create_reservations` the
  only store table that grows without bound — every interactive create ever made adds a row nothing deletes. Two
  follow-on debts, deliberately distinct: (a) digest the reservation fingerprint — rows currently retain enough of the
  original request to match retries, i.e. request plaintext (titles, cwds, invocations) retained forever; hashing bounds
  each row and ends the plaintext retention but does nothing about row count; (b) expiry/pruning — actually bounds the
  count, at the deliberate cost that a pruned key becomes reusable after the horizon (a very late retry could
  double-create). The digest half is worth doing under either verdict. If the verdict bounds the scope (session-lifetime
  — defensible, since a retry outliving the session it protects is protecting nothing), the pruning half mostly
  evaporates; that reverses a durability-milestone decision, so record the reversal where that decision lives. The store
  module's own docs describe both debts.

- Run the review-cap residue pass: one targeted review (test-quality and docs lenses only) over the three largest M7
  surfaces — auth, provisioning, packaging. During the M7 stack's reviews, the test-quality lens (often docs too) was
  still producing ACCEPTED findings at the hard three-pass cap on every large PR (#114, #115, #117, #118, #119/#120,
  #121, #122, #123) — those reviews ended because the budget ran out, not because the reviewers ran dry, so there are
  almost certainly real accepted-grade findings never surfaced in security-critical code. Treat a pass that returns zero
  accepted findings as saturation finally reached; anything it does return gets the normal fix-or-reject treatment.
  Cheap to run, and the difference between "reviewed until done" and "reviewed until the meter ran out" — do it before
  declaring the first real release final.

- Close the HostId-reuse create-default window. The create dialog defaults its host field to the selected session's host
  BY ROW ID, but a host row id survives a retarget (or an adopt where a new install takes over) while the machine behind
  it changes: look at a session on the old machine, have the row retargeted, open the create dialog within one
  listing-refresh interval, and the create lands on the successor install. The request's own install-incarnation check
  passes — the request was genuinely built against the successor — so the system does what it was told while the user's
  intent lands on the wrong machine. Raised as a definite security finding in #156's review; accepted there as residual
  because selection reconciliation already narrows the window to one refresh interval. The full fix: the helm's listing
  must denormalize install identity per session so the client binds its create default to the install the user was
  actually looking at, not the row id. Not urgent (needs a concurrent retarget plus a one-interval race), but "accepted
  residual" should not quietly become "permanent".

- Run the manual Mac checklist (`docs/manual-mac-checklist.md` — that file IS the record; its "Observed:" fields are the
  state, all "not run"). Blocked on a human with a real Mac. Not covered by any CI: Playwright's WebKit is not
  WKWebView.

- Decide whether several helms sharing one supervisor becomes supported, and what that requires. SPEC.md says concurrent
  helms are unsupported in v1, with the supervisor's one-attachment-per-session rule as the only backstop. Observed on
  2026-08-27 while acceptance-testing the 0.1.0 rc: a desktop helm (0.1.0-rc.1) and the browser helm (0.0.3) both
  registered the same host and both listed the same sessions, live, with no disconnects, and switching between the two
  surfaces worked. That is not luck — sessions and their status are supervisor-owned and the helm's `session_cache` is
  an explicit mirror, so any helm reaching the supervisor sees the same list. What was deliberately NOT tested: opening
  the SAME session in both helms. The expected result is the displaced-client path the spec defines for a second client
  (snapshot plus take-control, and auto-reconnect never seizing), since the supervisor enforces that rule, but the path
  has only ever been exercised between two clients of one helm. Known gaps before this could be called supported: (1) D2
  version coupling — each helm expects the supervisor at its OWN version and offers `update` otherwise, so helms of
  different versions would tug the host up and down (the rc helm already offered to "update" the 0.0.3 production
  supervisor; a compatibility rule such as "at least mine" plus a protocol version is design work, not a fix); (2) no
  lock against two helms provisioning or updating the same host at once; (3) the cross-helm takeover,
  replay-after-takeover and dimension handoff have no tests; (4) SPEC.md and SPEC_impl.md would need to state the
  supported model. Same-version helms look like a small step; mixed versions are the real work. First action when
  returning: run the untested case with two same-version helms and record what the displaced side shows.
