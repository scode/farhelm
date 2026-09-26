# TODO

A running list of things the maintainer wants fixed or built. This is intent, not history: an entry is REMOVED in the
same PR that addresses it, so the file only ever describes what is still wanted. It is not a roadmap and carries no
priorities unless an entry says so itself.

Eleven buckets, assigned by the maintainer: "definite simplification" is complexity the maintainer has decided to remove
— the decision is made, only the work remains; "planned" holds accepted work to implement later and suppresses duplicate
review triage within each item's stated scope; "near term" is what should be picked up next; "doc todo" holds
documentation work; "tricky bugs" retains unresolved bug reports and their investigation findings; "deflake" gathers
test and harness reliability work, including CI execution and restoring gates; "broken tests" records tests that fail
deterministically, with the failure and the evidence that it predates any in-flight work; "code review" is the residue
of the September 2026 review swarms after the policy pass, ordered by confidence and risk; "code cleanup" holds
structural code smells from a whole-codebase assessment, grouped by kind with a suggested order; "maybe later" is wanted
but not soon, and may never happen; "unbucketized" is everything not yet sorted, which carries no implication either
way. Within a bucket, no order unless the bucket explicitly says so.

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

- **Claude foreground ownership.** Assess whether native or shelled-out Claude children can replace or withdraw the
  foreground conversation's restart target, then define the smallest admission check that preserves legitimate
  clear/new/switch/fork/resume transitions. This is an assessment task, not a claim that every vendor path has been
  reproduced.
- **Pi foreground ownership.** Assess whether native or shelled-out Pi children can replace or withdraw the foreground
  conversation's restart target, then define the smallest admission check that preserves legitimate foreground
  transitions. This is an assessment task, not a claim that every vendor path has been reproduced.
- **OMP foreground ownership.** Assess whether native or shelled-out OMP children can replace or withdraw the foreground
  conversation's restart target beyond the checks now landed in #814, and define any remaining smallest admission check.
  Preserve legitimate foreground transitions and avoid extending the reporter's scope without evidence.

The earlier cross-harness evidence is preserved in
[the historical ownership assessment](lore/2026-09-20-harness-conversation-ownership.md).

## Doc todo

- Bring the README overview/splash content into the main documentation.
- Document the harness support feature matrix so supported and unsupported features are clear.
- Answer "Is it vibe coded?" with a clear explanation.

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

- **Replace refusal row in WebKit.**
  `Replace confirms inline, can cancel, selects the fresh session, and surfaces refusal` in
  `e2e/tests/terminal-restart.spec.ts` timed out locating the injected refusal row in full run
  `bd3fa658-dcf7-4820-8b68-67be5e4b89ed`, then passed unchanged on both engines in focused run
  `8b2b2c31-7608-4b6d-97b7-b11353f26554`. Cause unknown; inspect listing interception and refresh delivery on
  recurrence.

- **Browser stack parent-SIGTERM cleanup.** `scripts/test-start-stack-cleanup.sh` left the stack serving, with state and
  processes intact, after killing its spawner with SIGTERM in run `f2355071-3c67-4a7b-ba05-37f853c4a6b3`. The isolated
  repetition `a1b6e9b1-a246-4272-8d7b-89452a3f4c45` passed unchanged. Capture watcher and startup-process state at the
  failed cleanup boundary before changing the teardown contract.

- **Composer viewport controls.**
  `composer keeps launch and cancel inside the initial viewport at default and narrow width` in
  `e2e/tests/sidebar.spec.ts` failed during browser run `946f3bb1-6398-4bbc-9ece-7ab6232be092` and passed unchanged in
  focused run `51f444c3-d0db-4e33-bd27-32b6d429c3a7`. Reproduce with retained geometry measurements; the interrupted
  original run did not retain the final assertion report.

- **Working-copy identity reconciliation under the workspace battery.**
  `working_copies::tests::reconcile_fails_closed_when_a_stranger_holds_the_destination_and_the_source_is_gone` failed
  once in retained full Rust run `131912d5-0ee2-4313-a204-38edf6fc942c` and passed in the exact-test rerun
  `c44ac1bf-bcce-49b0-8a44-5d9633c5172f`; investigate the concurrent filesystem premise before changing the
  reconciliation contract.

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

## Code cleanup

NOTE: This is not a bug list and not a demand for perfection. It is the output of a code-smell assessment of the whole
workspace at `1867aed` on 2026-09-25, deliberately skipping nits, and keeping only things with a real cost in bug risk,
change cost, or comprehension. Five agent reviewers covered the supervisor, the helm, the UI, the CLI with proto and
test tooling, and cross-crate duplication. Six claims were re-checked by hand against the code (the escaping sets, the
`has_session` text match, the helm client's `{other:?}` replies, the stale lock-order paragraph, the duplicated "Version
20" paragraph, and the product binary's `farhelm-teststate` dependency); the rest are the reviewers' reports, which were
told to verify against the code. Line numbers are as of the anchor commit and will drift. Efforts are agent judgments.

The dominant pattern is not sloppy code. It is the same small rule reimplemented in several places, where the copies
have since diverged. Those come first because they are latent bugs and mostly cheap. Suggested order: the diverged
copies as small independent PRs; then the handlers reply path and the `SessionEntry` split (contained, and they reduce
ongoing cost); then the shared store helper for both stores at once; then the big mechanical carve-outs of `core.rs`
(with `launch_reserved` and `reload_sessions`) and `CreateSessionForm`, at a time when no other stacks are touching
those files.

Checked and found fine, so nobody re-raises them: state-dir resolution, the tmux version floor, and frame I/O are each
in one place; supervisor state machines are proper enums and the helm actor loop is clean; every declared dependency is
used, and the duplicate versions in `Cargo.lock` all come from dioxus, gtk, and ring; `setup.rs` and `provisioning.rs`
are large mostly because of their tests.

### Oversized modules and functions

- **Supervisor `service/core.rs`.** Medium-high effort. 29k lines, 14.4k of them production; one `impl Supervisor` (from
  `core.rs:4536`) spans about 9.8k lines and 95 methods covering create validation, launch, relaunch and restart, tabs,
  GitHub checkouts, startup reload, and conversation reports. `launch_reserved` (`core.rs:8455`) is 1,226 lines,
  `reload_sessions` (`core.rs:5305`) is 782 with at least four phases, and `relaunch_into_terminal`, `restart_session`,
  `spawn_agent`, and `new_with_seams` are each 300-470. The M4.5 carve-out stopped short. Fix: continue it into
  `create.rs`, `relaunch.rs`, `tabs.rs`, `reports.rs`, `startup.rs`, `github.rs`, each with its own `impl Supervisor`
  and the tests beside it, and split the long functions into phase functions along the phases their comments already
  name.
- **Long supervisor handlers.** Medium effort. `handle_restricted_control` and `handle_attach` in
  `farhelm-supervisor/src/service/handlers.rs` are each several hundred lines. Splitting them into phase functions goes
  with the held-out `core.rs` carve-out; the reply boilerplate, the 21-argument create call, and the copied credential
  check that shared this entry were cleaned up in the 2026-09 stack.
- **Per-agent-kind behaviour smeared across the supervisor.** Medium-high effort. About 114 non-test `AgentKind::`
  sites, a third of them in `core.rs` and `handlers.rs`; kind-specific argv surgery lives in `core.rs`
  (`with_hook_argv_using`, `goose_launch_shape`, `pi_interactive_invocation`); `report_codex_conversation`,
  `report_grok_conversation`, and `report_omp_conversation` (`core.rs:13556`, `:13708`, `:13865`) are about 150 lines
  each with the same skeleton, alongside parallel `*_foreground` and `refresh_*_capture_claimed` helpers and a
  coexisting legacy admission path. Adding or changing a kind is a shotgun edit. Fix: a per-kind trait or table in
  `agent_kind` for hook argv, locator verify, and foreground lookup, leaving one generic report pipeline in core. Held
  out of the 2026-09 cleanup stack as a large move through `core.rs`; the harness-to-kind table
  (`LaunchHarness::agent_kind`) that was its first step is done.
- **Helm `HelmStore`.** Medium effort. About 6.2k production lines, one struct with about 65 methods over nine concerns
  (web tokens, device sessions, preferences, seen table, host registry, session cache, create and launch history,
  profiles, remembered default). `apply_schema` (`farhelm-helm/src/store.rs:1508`) is one ~1,100-line function whose
  version-history doc stops at v17 while migrations reach v30. `record_create_history_with_destination` (`:4813`, ~410
  lines) pastes the same SQL "is newer" tie-break predicate six times in one upsert beside a Rust twin
  (`history_order_is_newer`, `:106`), behind a three-layer wrapper chain with a positional bool. Session list filter and
  sort types live in the storage layer. Fix: split into per-concern `impl HelmStore` modules as `checkout_config.rs`
  already does, one function per migration, a single tie-break fragment, per-table helpers sharing the transaction.
- **UI `CreateSessionForm`.** High effort. `farhelm-ui/src/list/create_form.rs:1431-5150` is one component with 62
  signals and a ~2,200-line markup body; the submit handler runs inline, and `apply_composer_search_result` (`:148`)
  takes 28 parameters. Seeded fields are spread over three signals each (`cwd`/`cwd_raw_seed`/`cwd_edited`, likewise
  invocation, title, model) where `profiles.rs` already uses one `ProfileDraft` struct; the folder lives in both `cwd`
  and `destination_draft` and submit reads one then overwrites from the other (`:3170`, `:3203`); and the create target
  is computed three times because `ListView` derives it in a `use_effect` (`list/view.rs:882-907`) with a documented
  one-render lag the form then patches over (`:1653-1661`). Fix: a shared `SeededField` type, one source of truth for
  the folder, a `use_memo` for the target in `ListView`, then split destination/browse, composer search, and submit out
  of the component.
- **UI `ListView` per-row state.** Medium effort. Seven parallel `HashSet<String>`/`HashMap<String, _>`/`Option`
  collections (`list/view.rs:583-694`) are recombined into each row's state, and the "is this row locked" triple is
  passed to five call sites; exclusivity rules such as confirm-delete versus confirm-replace hold only by call-site
  discipline. Session, tab, and profile ids are bare strings (`api::close_tab(base, session_id, tab_id)` can be called
  with the two swapped). Fix: one `HashMap<SessionId, RowPhase>` and id newtypes like the existing `HostId`.
- **CLI `main.rs`.** Medium effort. `main()` is ~540 lines (`farhelm/src/main.rs:688-1225`) mixing dispatch with env
  reads and the hook-log path derivation copied between `Hook` and `GooseHook`. `spawn_session` and `agent_request` each
  hand-roll connect, handshake, one request, one reply, but treat failures differently: `spawn` prints the supervisor's
  message unescaped and gives no "outcome unknown" warning on a lost reply to a mutating create, where `agent create`
  does. About 400 lines of table rendering live alongside. Fix: an agent-client module with one `one_shot_request`, a
  render module, and a `SessionEnv::from_env()`.
- **`desktop.rs` bundle.** Low-medium effort. ~2,450 production lines covering window geometry, the asset server, device
  token exchange, supervisor lifecycle, tmux preflight, clipboard, and state. `write_window_state` (`:701`) and
  `write_state` (`:2428`) are diverged copies of the atomic-write helper (only one fsyncs the parent, only the other
  cleans up its temp file and avoids the rename behaviour its own doc warns about). The desktop cfg predicate is written
  out 30 times. Fix: split into submodules, one `atomic_write_json`, and a cfg alias.

### Duplicated infrastructure

- **SQLite store plumbing, twice.** Medium effort. Both stores hand-write `Arc::clone` → `spawn_blocking` →
  `lock().expect(..)` about 123 times (helm 70, supervisor 53) and separately reimplement busy timeout, open flags with
  the 0600 fix, and the `user_version` migration chain with its fresh-equals-migrated test. They have drifted:
  `foreign_keys` is on only in the helm, and only the helm has an open-existing-never-migrate mode. The supervisor's
  `working_copies` also queries the store's connection directly, so table ownership is split. Fix: a shared
  `Db::call`/`with_tx`, `open(path, OpenMode)`, and migration runner, with consistent pragmas.
- **Two helm session caches with one set of rules.** Medium-high effort. Hosts with an identity cache sessions in SQLite
  (`store::remember_session`, `farhelm-helm/src/store.rs:4575`), hosts without keep them in memory
  (`manager::remember_session`, `manager.rs:1834`), and merge, sort, cap, eviction, and the truncated flag are written
  once in SQL and once in Rust; `forget_session` and `refresh_once` each branch on identity. Helpers point the wrong way
  (store calls `manager::merge_cached_session`, manager calls `sessions::resolve_session_profiles_from_store`), and the
  session id length bound is checked in five places. Fix: one pure policy module both backends call, or a cache type
  with two backends behind the manager.
- **Subprocess runners.** Medium-high effort. At least four timeout-and-cap runners with different kill semantics:
  `farhelm-supervisor/src/tmux.rs:663` (sync, process-group kill), `farhelm-helm/src/provisioning/backend.rs:1474`
  (async, process-group kill), `farhelm-supervisor/src/repository_discovery.rs:234` (async, output cap, no process
  group), and `farhelm-teststate/src/process.rs:234`; `farhelm-supervisor/src/scope.rs:1271` uses bare `.output()` with
  no cap. Fix: one async runner with process-group kill and output caps as options. Payoff grows with each new caller.
- **Hand-rolled fake supervisors in tests.** Medium effort, test code only. About 180 inline duplex + handshake +
  hand-matched reply setups in the helm (`sessions_tests.rs` ~65, `client.rs` 49, `agent_requests.rs` 23, `uploads.rs`
  19, `terminal.rs` 13) while only `manager.rs` has a reusable scripted peer; about 16 more in the e2e tests (`RawPeer`,
  `MarkerPeer`, `SessionPeer`, seven in `session_lifecycle.rs`); and the CLI mock supervisor with its four self-tests is
  copied between `tests/agent_cli.rs` and `tests/spawn_cli.rs` despite `tests/cli_support/`. A wire or handshake change
  fans out across hundreds of test bodies. Fix: a shared scripted fake in `rest_harness`, a `harness::raw_peer`, and the
  CLI mock moved into `cli_support`.

### Invariants held by convention

- **`SessionEntry` sharing policy.** Medium effort. The 14-field struct (`farhelm-supervisor/src/service/core.rs:3734`)
  is built literally at six production sites, each re-deciding which `Arc` cells a rename shares versus which a relaunch
  replaces; the doc around `:2998` warns the next person to decide correctly, and a wrong choice breaks the generation
  fence. The cells are locked directly from core, capture, terminals, and ticker. Fix: split into `SessionCells` and
  `RunCells` behind two `Arc`s, so rename clones both and relaunch replaces one, with accessor methods.
- **Supervisor lock ordering.** Low-medium effort. With four mutexes, four keyed locks, working-copy operations, and
  three semaphores, the order is written down piecemeal in about eight comments (`core.rs:4146`, `:4296`, `:4350`,
  `:4382`, `:4426`, `:9850`, `handlers.rs:1422`, `teardown.rs:94`), and the struct doc at `core.rs:4019` still says
  attachments-before-sessions is the only rule needed. Fix: one authoritative lock-order table, the stale paragraph
  corrected.

### Test hooks in production code

- **Fault hooks mixed into configuration.** Medium effort. `SupervisorSeams` (`core.rs:662-1004`) has about 36 public
  fields, about 25 of them test fault or gate hooks alongside real config; six type aliases name the same gate type;
  more `#[cfg(test)]` fields and branches sit in tmux, scope (`Mode::Fake`), repository discovery, stream, sink, and
  `agent_relay.rs`. The helm grows one bespoke seam per test (`fail_registry_sync`, `fail_before_rename`, the
  `DUPLICATE_PUBLICATION_GATE` global static, `*_for_test` store methods). The UI ships 32 `window.__farhelmTest*` hooks
  and test-observation signals. Fix: a `SupervisorConfig`/`FaultHooks` split behind a `test-seams` feature the e2e crate
  enables, one failpoint mechanism in the helm, and a `test-hooks` feature for the Playwright build.
- **Test fixtures in the release binary.** Medium effort, lower payoff. `internal fake-agent` (~3.8k lines with
  `codex_conversation.rs`) and `internal sweep-test-state` ship in the product binary, and `crates/farhelm/Cargo.toml`
  depends on `farhelm-teststate` although that crate's docs say product code must never depend on it. Fix: a cargo
  feature or a second `[[bin]]` for the e2e harness and `start-stack.sh`.

### Stale in-code documentation

- **Split proto `lib.rs` by message family.** Low effort, mechanical. `farhelm-proto/src/lib.rs` holds every wire type
  for every message family in one file that every protocol change touches (about 3.5k production lines plus tests). Held
  out of the 2026-09 cleanup stack as a large mechanical move to time when no other stacks are touching proto; the
  changelog docstring and the cross-protocol test pruning that shared this entry are done.

## Maybe later

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
