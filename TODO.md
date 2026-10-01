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

- **Killed-shim checkout-preparation test leaks processes when it fails.** In `crates/farhelm-supervisor/src/launch.rs`,
  `d4_killed_shim_never_repeats_the_hook_or_spawns_the_agent` starts the preparation shim, whose preparation hook parks
  on a FIFO (`build_d4_fixture`), checks the hook is running under the shim, that it was called once, and that the
  durable state is `HookStarted`, then SIGKILLs the shim on purpose, reaps it, and only afterwards writes the release
  byte to the FIFO. Every one of those checks is an `assert!` that runs while the shim and hook are alive, and the shim
  is held as a plain `std::process::Child`. If any of them fails (including the 30-second readiness poll), unwinding
  drops that handle without killing or reaping anything: the shim stays waiting on the hook, and the hook stays blocked
  reading the FIFO. Deleting the fixture's temporary directory does not unblock it, because the hook opened the FIFO for
  both reading and writing. A failure after the deliberate kill but before the release byte orphans the hook the same
  way. The processes then live until someone kills them, which on a shared machine is exactly the stray load that
  FLAKES.md records tripping other tests' timing budgets. A passing run is unaffected. The existing `ReapedPrepChild`
  wrapper is not enough on its own: it kills and reaps the shim but not the hook the shim started. The fix needs an
  unwind-safe owner for both from the moment the shim is spawned, for example starting the shim in its own process group
  and killing and reaping that group on drop, while keeping the mid-test SIGKILL aimed at the shim alone so the
  interruption the test exists for still happens. Found by the review of the PR that moved these tests' markers out of
  `/tmp`; the gap predates that PR.

- **Alarming link hover when the text and target disagree.** Hovering a hyperlink a program printed (OSC 8) shows its
  host and full target in a small, quiet display (`showLinkTarget` in `crates/farhelm-ui/assets/terminal-links.js`,
  added in #1158). Keep that display for every such link, but when the underlined text does not match where the link
  actually goes, make it much more intrusive (big, red, blinking, or something along those lines) so the mismatch is
  hard to miss before clicking. A difference only in a trailing slash does not count as a mismatch (the text
  `http://127.0.0.1:6080` whose target displays as `http://127.0.0.1:6080/` after URL normalization, or `/docs` versus
  `/docs/`): it is very common and almost never matters, and flagging it would teach people to ignore the warning. Other
  details are to be worked out, including how to treat link text that is not a URL at all (a file name, "#123", "click
  here"), which never matches but is not the lookalike case this is meant to catch. SPEC.md (Terminal experience)
  currently calls the hover display the whole safeguard, so it changes with this.
- **Coordinate uninstall with installation, setup, and runtime startup.** SPEC.md's "Concurrent and interrupted runs"
  now requires a correct outcome (refusing is fine) when uninstall overlaps installation, updates, setup, desktop
  startup, or session creation; the earlier carve-out that assumed these never overlap was dropped on 2026-09-28. Share
  the relevant locks and revalidate removal targets under them, or refuse, so none of these can race uninstall's checks
  and deletion. Prefer the simplest mechanism that gives a correct result.
- **Keep sessions from stumbling into the private tmux.** SPEC.md now says the private tmux server is an implementation
  detail and that interacting with it directly is unsupported ("Ownership during cleanup and provisioning"). But every
  agent terminal and tab inherits `TMUX` pointing at that server, so a plain `tmux new-window` or `tmux split-window`,
  typed by the user or run by an agent (for example to start a dev server in its own window), lands in Farhelm's tmux
  instead of failing or reaching the user's own tmux. Whatever it starts there escapes Stop, Restart, Delete and tab
  close. Find out how to make that accidental path unlikely without adding real complexity: unsetting `TMUX` (and
  `TMUX_PANE`) in the agent and tab environments is the obvious candidate, but check what it breaks in Farhelm's own
  launch and tab paths, and whether running tmux then silently starts or attaches to the user's own default server,
  which may be just as confusing. This came up in review-feedback triage on 2026-09-28, from findings that agent-opened
  tmux windows and hand-split tab panes escape cleanup; the decision was to declare that use unsupported rather than
  reap it, with this as the follow-up to make it hard to do by accident. Deliberate access, such as pointing tmux at the
  socket explicitly, stays out of scope.

- **Accept a leftover uninstall receipt after the install directory moves.** On macOS, an interrupted
  `farhelm uninstall` leaves `~/Applications/.Farhelm.app.uninstall-receipt`, a copy of the bundle's ownership record,
  so a retry can finish. Before building `Farhelm.app`, `scripts/install.sh` checks that receipt with
  `record_file_is_ours "$pending_receipt" "$pir_canonical"`, an exact match of the directory the record names against
  this installation's canonical directory, and refuses the bundle step for any other receipt with "was left by an
  interrupted farhelm uninstall and is not this installation's record ... (or delete it if that installation is gone)".
  The bundle's own record gets a wider rule (#1278): `bundle_record_moved_here` also accepts a record naming a directory
  that now resolves to this one (the old `~/.local/bin` replaced by a symlink, a renamed home) or that provably no
  longer holds an installation (`path_provably_absent`), with `bundle_record_dir` parsing the record. The receipt check
  never got that rule, so after such a move an interrupted uninstall's receipt is refused as foreign, and the message
  points the user at "that installation", which is this one. Fix: give `bundle_record_dir` a variant that takes the
  record file's path (as `record_file_is_ours` was split out of `bundle_record_is_ours`), accept the receipt when the
  moved-here rule accepts it, and delete it after the rebuild the same way an exact-match receipt is deleted today.
  Cover it in `scripts/test-install-sh.sh` next to the existing leftover-receipt cases, and update the leftover-receipt
  paragraph in `docs/install_uninstall.md`. Low severity: the refusal is over-cautious, never destructive, and deleting
  the receipt by hand recovers. Found while rebasing the review-feedback stack onto #1278.

- **Decide whether teardown should wait out the systemd probe's timeout backoff.** When the supervisor's probe of the
  systemd user manager runs out of time, `crates/farhelm-supervisor/src/scope.rs` caches `Verdict::TimedOut` and, for
  `TIMED_OUT_REPROBE_INTERVAL` (60 s), both `ScopeManager::available()` and `reprobe()` answer false without asking the
  manager again, even for a caller holding scope evidence (#1279). Stop, Restart, Delete and tab close treat a scope
  they know the launch had but cannot check as an unconfirmed cleanup (SPEC.md "Lifecycle operations", confirmed
  2026-09-28): `reap_process_tree` in `service/sweep.rs` turns each recorded unit into "scope ... could not be checked
  because this host's systemd user manager is not usable now", and `ScopeKillFailure::Refuse` fails the operation. So
  for up to a minute after a probe timeout, those operations fail on every scoped session and tab, and the ticker's
  `reap_dead_tabs` fails and warns on each tick for a dead scoped tab, spending that tick's tab-reap budget. A retry
  after the window works. The question: is that acceptable, or should teardown with durable scope evidence (a unit the
  session row recorded, a tab window marked as opened in a scope) probe again at once despite the backoff? Re-probing
  costs up to the probe's 15 s bound per teardown while the manager stays slow, which is what the backoff exists to
  avoid. Whatever is chosen, record it in SPEC_impl.md's scope paragraph ("The manager is probed once and the answer
  cached, with two exceptions ...").

- **Run the host alias edit on a helm-owned task.** `set_alias` in `crates/farhelm-helm/src/hosts.rs` commits the new
  alias (`store.update_alias`) and then calls `manager.sync_registry()`, both on the request's own task. The reconcile
  is what announces the edit on the event feed (see the "Only the alias is compared" comment in `sync_registry` in
  `manager.rs`), so a client that disconnects between the two leaves the alias saved while every other open client keeps
  showing the old one until an unrelated reconcile runs (another host add, edit or removal, or a helm restart). That is
  the commit-then-follow-up shape SPEC_impl.md "Who owns an accepted action" rules out. Add, retarget, remove, adopt and
  the YOLO-safe toggle already run their bodies through `crate::run_owned`; give `set_alias` the same shape as
  `set_destination` (a thin handler calling `crate::run_owned(set_alias_owned(state, host, spec))`), keeping its
  provisioning lock and write lock inside the owned body. The review-feedback triage that fixed the other host edits
  (`host-edits-not-cancellation-safe`) did not list the alias edit.

- **Confirm restart only while the agent is working.** Restart (from the session header or the sidebar) asks "still
  running — restarting stops the agent and its whole process tree first" whenever the agent is live at all, idle
  included, and also when its status is unknown. Asked that often, people click through without reading it. Only ask
  when there is signal that the agent is actively doing work; an idle agent, one waiting for input, or one whose status
  is unknown restarts without a prompt. Accepted consequence: for harnesses whose activity detection is weak, the user
  may not get the warning even when the agent is busy. This is not UI-only: SPEC.md (Lifecycle operations, Restart) says
  a restart of a still-running agent confirms, and the supervisor refuses an unconfirmed restart of a live agent, so
  both change with it. Replace keeps its confirmation as it is: it discards the conversation, whatever the agent is
  doing.

- **Name the YOLO host setting after what it does.** One per-host yes/no setting has two names: settings and the specs
  say "YOLO safe", while the YOLO confirmation, the helm's refusal and the command line say "sensitive". Both read as a
  judgment about the machine, and neither is what the setting stores, which is whether Farhelm asks before starting a
  YOLO session there. The settings checkbox label, "allow YOLO launches on this host", is also inaccurate: YOLO launches
  are allowed on every host, with a confirmation. Agreed wording (2026-09-30): the checkbox becomes "start YOLO sessions
  here without asking" (off by default); prose says a host "asks before YOLO launches" or allows "YOLO without asking"
  instead of "sensitive" and "YOLO safe"; the confirmation heading becomes "Confirm YOLO launch"; and
  `--allow-yolo-on-sensitive-host` becomes `--confirm-yolo`, keeping the old name as a hidden alias so existing scripts
  and agent instructions keep working. The helm's stored field and the wire field name stay as they are: users never see
  them. Covers the helm's refusal sentence, the CLI flag and its help, SPEC.md, the docs website, and the GUI text; it
  needs a changelog entry for the flag. Split out of the pre-release UI work that added the "don't ask again" button to
  the YOLO confirmation, which may already use some of this wording.

- **"Replace with" a gh: checkout refused because the checkout path exists.** Using "replace with" to switch a session
  to a `gh:` fresh checkout was refused with an error saying to pick a different session name because the git checkout
  path already exists. Not yet investigated: it may fail like that every time, or something subtler about that session's
  state may have triggered it. Reproduce first, then fix whichever it turns out to be.

- Make `install.sh`'s output easier to scan. The completion message is a wall of text mixing installation results,
  restart instructions, and setup advice. Improve the layout and visual hierarchy, possibly with color; details TBD.

- Guard provisioning against pushing payloads older than the helm's own protocol. A release-shaped helm built from a
  commit newer than the latest release (the local stable-binary flow does exactly this) provisions remote hosts with
  DOWNLOADED released payloads by default (D13), so the freshly provisioned supervisor can speak an older protocol than
  the helm that just installed it — and the helm then refuses it at the hello gate. Nothing is damaged (the refusal is
  the version rule working), but the failure arrives one step late, as a skewed host instead of a refused provisioning
  attempt. Possible shapes: compare the payload's version against the helm's `PROTOCOL_VERSION` before pushing and
  refuse with a message naming the mismatch; or make the staged-payload path (`--payload-dir`,
  `FARHELM_HELM_PAYLOAD_DIR`) the documented answer for from-main helms. Noted 2026-08-31 when upgrading the stable
  install to a from-main build while the newest release was still 0.1.1.

- **No network path for the desktop app.** The desktop app's webview talks to its embedded helm the way the browser
  does, over HTTP and WebSockets on a loopback port, so one UI code path serves both clients. That port can be reached
  by every process on the machine, including other accounts' (the credential stops them from using it), and the helm's
  browser defenses need a scheme-level exemption for the webview's `dioxus://` origin, which every Dioxus desktop app
  shares. Consider moving the desktop client to an in-process transport instead, as Tauri commands or Electron IPC do:
  with nothing listening, nothing else on the machine can reach the desktop's API, and neither the port nor the origin
  exemption is needed. The hard part is streaming: wry's custom-scheme handler answers each request with one complete
  response, so terminal output and the event feed would have to travel over the webview's IPC channel, and the UI's
  network layer would need a second transport beside HTTP.

- **Claude Code reads as idle while it waits on background agents.** When Claude Code's main turn has ended but
  background subagents it launched are still running, it is still working, and Farhelm should show the session as active
  rather than idle. Captured from a real session (Claude Code with background reviewer subagents, model shown as Opus
  5.5; user, host, and path lines left out). The bottom of the screen read, top to bottom:

  ```
  ● Agent "correctness-state-lifecycle review" finished · 14m 22s

  ● The state-lifecycle reviewer is done. [... several lines of the main agent's reply, ending:]
    Once it reports, I'll merge everything and hand the findings to the restater to rewrite for readers who don't know the code.

  ✻ Waiting for 5 background agents to finish
  ──────────────────────────────────────────────
  ❯
  ──────────────────────────────────────────────
    [user's custom status line]
    ⏵⏵ bypass permissions on · 1 shell · /tasks to see subagents · ← for agents

    ● main
    ○ general-purpose (+3)  Reading SPEC.md desktop Quit section          18m 5s · ↓ 489.8k tokens
  ```

  The signals are the dim spinner line "Waiting for N background agents to finish" just above the empty input box (the
  leading glyph is Claude Code's animated spinner, captured here as `✻`), the "/tasks to see subagents · ← for agents"
  hints in the footer, and the agent list under it, where `○ general-purpose (+3)` shows a running subagent's current
  step, elapsed time, and token count, listed under `● main`. Note that the spinner said 5 agents while the list showed
  one entry plus 3 more. The supervisor's Claude screen reader is tested against real screens in
  `crates/farhelm-supervisor/tests/fixtures/screens/claude/` (see `docs/agent-screen-fixtures.md`); a fixture of this
  state belongs there.

- **Ambiguous planned checkout test depends on the runner's real systemd.** In
  `crates/farhelm-supervisor/src/service/core.rs`, `an_ambiguous_planned_checkout_never_adopts_a_foreign_directory`
  failed the v0.20.0 release gate (GitHub Actions run `36811756125`, recorder run
  `80893b36-8051-45bd-97f4-9ed8e8b451fb`) at its final assertion, "Delete retires the committed refusal without a
  restart". Its trace shows the supervisor's systemd user-manager probe getting no answer for its full 15 s, after which
  the teardown ran on the sweep-only path and Delete did not succeed. The same code passed the v0.20.0-rc.4 gate and 20
  of 20 local repetitions (hunt batch `dda5c41c-7b4e-4b78-af86-23bc4999924a`), where the local manager answers at once.
  It is the class FLAKES.md recorded on 2026-09-29 and #1228 fixed for two handler tests by giving their fixture a
  disabled scope manager; this test still builds its supervisor with the real one. First step: decide whether the test
  needs scopes at all, and if not give it the disabled scope manager the way #1228 did; separately, find out why Delete
  fails on the sweep-only path, since that is reachable in production when the manager is slow.

## Doc todo

- Bring the README overview/splash content into the main documentation.
- Document the harness support feature matrix so supported and unsupported features are clear.
- Answer "Is it vibe coded?" with a clear explanation.
- Tell users that on Codex sessions Farhelm starts with hook injection, hooks Codex has not reviewed still run: those in
  their Codex config home and, once a workspace is trusted, the workspace's own `.codex/` hooks. Trusting a workspace
  means trusting its Codex configuration to run commands. SPEC_impl.md's hook-injection section states the rule.
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

The dominant pattern was not sloppy code but the same small rule reimplemented in several places, with the copies since
diverged. A 2026-09 stack of small PRs removed those copies (escaping, quoting, tmux refusal parsing, wire types, detach
reasons, the handler reply path, the store plumbing, the session-cache rules, the subprocess runner, and the lock order)
and is not listed here. What remains is of two kinds: the large mechanical moves that were held out to be timed when no
other stack touches the same files (the `core.rs` carve-out with its handlers and per-kind behavior, the `HelmStore`
split, `CreateSessionForm`, and the proto split), and the follow-ups that stack deliberately left, each entry saying
where it stopped.

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
- **Split `HelmStore` by concern.** Medium effort, mostly mechanical. `farhelm-helm/src/store.rs` is one struct with
  about 65 methods over nine concerns (web tokens, device sessions, preferences, seen table, host registry, session
  cache, create and launch history, profiles, remembered default), `apply_schema` is one ~1,100-line migration ladder,
  and session-list filter and sort types live in the storage layer. Fix: per-concern `impl HelmStore` modules as
  `checkout_config.rs` already does, one function per migration, and the filter and sort types moved beside the list
  code. Held out of the 2026-09 cleanup stack as a large move; the duplicated tie-break SQL, the create-history wrapper
  chain and helpers, and the missing v18-v30 migration history that shared this entry are done.
- **UI `CreateSessionForm`.** High effort. `farhelm-ui/src/list/create_form.rs:1431-5150` is one component with 62
  signals and a ~2,200-line markup body; the submit handler runs inline, and `apply_composer_search_result` (`:148`)
  takes 28 parameters. Seeded fields are spread over three signals each (`cwd`/`cwd_raw_seed`/`cwd_edited`, likewise
  invocation, title, model) where `profiles.rs` already uses one `ProfileDraft` struct; the folder lives in both `cwd`
  and `destination_draft` and submit reads one then overwrites from the other (`:3170`, `:3203`); and the create target
  is computed three times because `ListView` derives it in a `use_effect` (`list/view.rs:882-907`) with a documented
  one-render lag the form then patches over (`:1653-1661`). Fix: a shared `SeededField` type, one source of truth for
  the folder, a `use_memo` for the target in `ListView`, then split destination/browse, composer search, and submit out
  of the component.

### Duplicated infrastructure

- **Working-copy tables live in two modules.** Low effort. The supervisor's `working_copies` module runs its queries on
  `SessionStore`'s connection directly (for example `origin_working_copy(&conn, ..)` from inside store methods), so
  which module owns those tables' schema and invariants is split. Fix: move the working-copy SQL into a store submodule,
  or give `working_copies` a narrow trait over the store. The shared connection plumbing that used to share this entry
  (`farhelm_supervisor::db`) is done; the migration ladders stay per store with the held-out HelmStore split.
- **Two helm session caches behind one interface.** Medium effort. Hosts with an identity cache sessions in SQLite,
  hosts without keep them in memory, and the rules both follow (reply merge, creation order, cap eviction, id bound) now
  live in `farhelm-helm/src/session_cache.rs`. What remains is the plumbing: `remember_session`, `forget_session`, and
  `refresh_once` in `manager.rs` each branch on whether the host has an identity and carry a storage-specific body per
  branch. Fix: a cache type with the two backends behind it, so the manager calls one interface.
- **Hand-rolled fake supervisors in tests.** Medium effort, test code only. Shared peers now exist and each is proven on
  one file: `rest_harness::FakeSupervisor` in the helm (`uploads.rs`), `harness::RawPeer` in the e2e tests
  (`attachment_uploads.rs`), and one CLI mock supervisor in `tests/cli_support/mock_supervisor.rs`. What remains is the
  migration: about 160 inline duplex + handshake + hand-matched reply setups in the helm (`sessions_tests.rs` ~65,
  `client.rs` 49, `agent_requests.rs` 23, `terminal.rs` 13, and two `uploads.rs` window tests that also need a bounded
  nothing-arrives read), and the other e2e peers (`MarkerPeer`, `SessionPeer`, seven in `session_lifecycle.rs`). A wire
  or handshake change still fans out across all of those.

### Test hooks in production code

- **Test hooks in the helm and UI.** Medium effort. The supervisor's fault and gate hooks now live in `FaultHooks`,
  compiled only for tests (`test-seams`). The helm still grows one bespoke seam per test (`fail_registry_sync`,
  `fail_before_rename`, `*_for_test` store methods), each already `#[cfg(test)]` but with no shared mechanism, and the
  UI ships 32 `window.__farhelmTest*` hooks and test-observation signals in the product bundle. Fix: one failpoint
  mechanism in the helm, and a `test-hooks` feature for the Playwright build.

### Stale in-code documentation

- **Split proto `lib.rs` by message family.** Low effort, mechanical. `farhelm-proto/src/lib.rs` holds every wire type
  for every message family in one file that every protocol change touches (about 3.5k production lines plus tests). Held
  out of the 2026-09 cleanup stack as a large mechanical move to time when no other stacks are touching proto; the
  changelog docstring and the cross-protocol test pruning that shared this entry are done.

## Maybe later

- **Consider dropping Linux support without a systemd user manager.** Today a Linux supervisor that finds no usable
  `systemd --user` falls back to the same portable process sweep macOS uses, with the weaker cleanup guarantee SPEC.md's
  lifecycle operations section describes. The macOS path must stay, because running on an ordinary MacBook is a goal,
  but on Linux the fallback mainly serves hand-run supervisors and hosts whose user manager is missing or broken.
  Dropping it would narrow the support and test matrix; the code saving is small, since the sweep itself is shared with
  macOS and only the `/proc` reader is Linux-specific. Open question before deciding: a probe that times out is already
  retried a minute later, but a definite "no usable manager" answer is cached for the supervisor's lifetime, so a
  systemd host whose user manager is briefly broken when the supervisor first probes keeps the weaker sweep until the
  supervisor restarts; dropping the fallback would mean refusing to launch there instead, or probing again. Came up in
  review-feedback triage on 2026-09-28.

- **Reassess the silent fallback when systemd is expected.** On Linux each session normally runs in its own systemd
  scope, which is what guarantees all its processes stop with it. Since #1279, a launch that meets a slow systemd user
  manager still starts, just without a scope, so that session relies on the weaker process-tree sweep for good; later
  launches check again. Reassess whether a host that is expected to have systemd should hard-require it instead: refuse
  the launch, or at least say so, rather than quietly falling back. Part of the question is how Farhelm knows a host is
  expected to have it. Decide together with the entry above on dropping the no-systemd fallback altogether, which covers
  hosts that never had a working user manager.

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
  exception from the host-isolation policy. The end state: only explicitly trusted environments may spawn sessions on,
  or interrogate the session and profile data of, other hosts; arbitrary attached supervisors lose both. In scope next
  to create, clone, and their retry paths: `ResolveProfile` (which hands any attached host any profile's full resolved
  launch bundle today) and the fleet-wide session and host listings. These operations currently let a remote host cause
  arbitrary execution on another host; this is explicitly accepted temporarily to defer redesign, not permission to add
  more such operations. Preserve the eventual ability for agents to orchestrate sessions across hosts through an
  explicitly authorized launch policy, potentially trusted profiles, without letting the requesting host choose
  arbitrary execution. Include the existing agent/supervisor-originated creation and retry paths: a delayed resubmission
  must be considered when deciding what launch authority remains valid, including whether a forgotten retry key can
  launch a session again. Permanent retention of these agent-originated retry records is not required; their replay
  exposure is accepted pending this work. Do not add further exceptions or infer a waiver of user-initiated GUI request
  correctness. Cross-host stop and rename remain intentionally allowed bounded operations.

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
