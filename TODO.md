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

- **Make the bundled terminal font a hard requirement.** Decided 2026-10-01: JetBrains Mono Nerd Font ships with
  Farhelm, so failing to load it is a broken deployment, like any other bundled asset, not something to survive. Remove
  the fallback in `crates/farhelm-ui/assets/terminal.js` ("Font settling before mount": the 3 s settle deadline,
  mounting in a fallback font, the late font swap and repaint paths, the do-nothing tracker for engines without the Font
  Loading API) and gate terminal mounts on the font the way `mountWhenReady` already gates on the other bundled files.
  The retry against the browser briefly reporting no face before the font is registered may still be needed. Accepted
  cost: in the web UI over a slow link, a terminal shows nothing until the roughly 2 MB font arrives instead of
  appearing in a fallback font after 3 s. Removes the cause of `terminal-font-promise-leak.md` (discarded 2026-10-01).

## Planned

- **Keep session creation off the connection read loop.** Run creation through tracked background handlers so ordinary
  launch work and admission waits do not delay terminal input or unrelated requests on the helm's shared connection to
  that supervisor. Cover both full-authority and session-authenticated create paths; preserve bounded handler admission,
  credential revalidation after waits, keyed-create durability, and disconnect cleanup. Verify with a deterministic
  regression that another request or terminal input progresses while creation is paused. Medium effort: localized
  dispatch changes, with care around task ownership and existing lifecycle guards. This plans the fix described in
  `create-runs-inline-on-read-loop.md`; it does not request immediate implementation.

## Near term

- **Remove heuristic conversation-identity fallbacks.** Decided 2026-10-01: Farhelm identifies an agent's conversation
  only from the harness's own explicit report (a hook, plugin, extension, or whatever reporting mechanism that harness
  needs), never from heuristics that cannot be relied upon. Remove Claude's record scan (`agent_kind/capture.rs`,
  `service/capture.rs`, and their e2e suites; roughly 6k lines) and the re-verification of records it captured earlier,
  plus any other heuristic identity fallback found along the way. Launches without a report (a profile already passing
  `--settings`, a bare `--`, `FARHELM_AGENT_HOOKS` opting out, a hook that failed) take the fallback SPEC.md already
  defines for an uncaptured identity. Review feedback that is only true because this code still exists is discarded
  rather than fixed. Alert the maintainer before landing if existing scan-captured sessions would lose a valid Resume
  offer. Earlier write-up: https://claude.ai/code/artifact/554790ce-c744-4daa-b9a5-151facdb1f42

- **Apply identity reports that arrive while the session's record is busy.** Decided in triage 2026-10-01: a
  conversation-identity report from the correct, verified source must never be dropped because it arrived late. Today
  report admission waits at most a second for the session's capture claim (`CAPTURE_CLAIM_WAIT` in
  `crates/farhelm-supervisor/src/service/core.rs`, and SPEC_impl.md's five-step admission promises that bound) and
  otherwise refuses; Claude's hook never resends and nothing else catches up, so after a `/clear` whose report lost the
  race, Resume reopens the cleared conversation. The complication found while planning the fix: simply waiting longer
  overruns the hook's own 2 s budget and shows a hook error in the user's terminal, and replying first and writing later
  breaks attribution, because Claude's check that the hook ran in the pane process or its direct child needs the
  reporting process still alive and it exits once answered. The design that works runs the read-only attribution and
  proofs before replying, outside the claim, then commits on a supervisor-owned task that takes the claim without a time
  limit, reloads, and commits through the existing generation-and-binding compare-and-swap; that reorders the documented
  admission steps and changes SPEC_impl.md. Codex, Grok and OMP share the same wait and should get the same rule. Review
  item: `review_feedback_queue/claude-clear-report-dropped-on-claim-timeout.md`.

- **Re-examine and simplify how launches are represented.** The maintainer wants to interrogate how launches are handled
  end to end and reconsider the design with simplification in mind. Today a session can be launched from a structured
  selection, a built-in profile, a user profile, or a raw command line, and a profile or raw create can carry a separate
  resume command alongside its start command; each path decides YOLO, resume and restart behavior its own way. Example
  of the resulting complexity: the sensitive-host YOLO check classifies only the start command, so a custom profile
  whose hand-written resume command adds `--dangerously-skip-permissions` passes the check at creation and every later
  Resume runs with approvals off unconfirmed (`yolo-guard-skips-resume-template.md`, triaged 2026-10-01 as a spec
  clarification rather than a code fix because closing it was not worth the complexity). Another: the resume command
  Farhelm derives by appending a conversation selector to the original command line collides with a selector the user's
  own command already carries, so a Claude session started as `claude --continue`, or with a `--` in its command, can
  restart into a different conversation or a fresh one (`claude-resume-template-selector-collision.md`, triaged
  2026-10-01; Codex had the same problem in `codex-resume-template-duplicates-selector.md` and now refuses such a launch
  at create time unless it brings its own resume command). First step: walk through the launch paths with the
  maintainer.

- **Uninstall after a move when the installer skipped the app.** On macOS, after the install directory moves
  (`~/.local/bin` replaced by a symlink, a renamed home, a different `FARHELM_INSTALL_DIR`), `farhelm uninstall` refuses
  until the installer is re-run from the new directory: the install directory's own ownership record, and the app's
  record (or the receipt an interrupted uninstall leaves beside it), name the old path, and the uninstaller requires
  each to name this installation's directory in its exact physical spelling (`field_path` and `inspect_bundle_at` in
  `crates/farhelm/src/uninstall/ownership.rs`; `receipt_paths_require_exact_physical_spelling` pins it). Re-running the
  installer rewrites both records (the app's by its moved-here rule, extended to the leftover receipt on 2026-09-30), so
  the documented remedy works. What is left: a re-run that skips or fails the app step (`FARHELM_NO_APP_BUNDLE=1`, a
  held or failed app lock) rewrites the directory's record but not the app's, and uninstall then refuses the app.
  Porting the installer's moved-here rule to the uninstaller was considered and stopped, since accepting an alias or a
  vanished directory in the step that deletes the app reverses that deliberate rule. Decide whether that leftover is
  worth a change, or drop this entry.

- **"Replace with" a gh: checkout refused because the checkout path exists.** Using "replace with" to switch a session
  to a `gh:` fresh checkout was refused with an error saying to pick a different session name because the git checkout
  path already exists. Not yet investigated: it may fail like that every time, or something subtler about that session's
  state may have triggered it. Reproduce first, then fix whichever it turns out to be.

- **Clone of a gh: checkout session reuses the same working copy.** Cloning a session launched on a `gh:` fresh
  checkout, then renaming the clone, still left the clone on the original session's working copy rather than a checkout
  of its own. The maintainer knows the cause and can fix it up by hand, but the clone flow for these sessions needs
  improving. Details TBD.

- **Clone then gh: should pick a fresh session name and checkout directory.** When the user clones a session and enters
  `gh:some/repo` as the clone's target, Farhelm should allocate a new session name and checkout directory for it. Today
  the default experience is an error saying the checkout conflicts with the existing one. Closely related to the entry
  above on clones reusing the original working copy. Details TBD.

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

- **Audio signal when an agent is waiting on input.** Play a sound when a session's agent gets stuck waiting for the
  user (a question, an approval prompt), so a user looking at something else notices without watching the sidebar.
  Possibly other events too; which ones, and the sound, volume, and any setting to turn it off, are to be decided when
  this is picked up.

- **Easy font size changes.** Let the user make the font larger or smaller with keyboard shortcuts, plus buttons for the
  same. Which shortcuts, where the buttons go, whether it covers the terminal, the rest of the UI, or both, and whether
  the size is remembered, are to be decided when this is picked up.

- **Download files named in the terminal.** When text in a session's terminal looks like a file path (an agent saying "I
  wrote the file here"), hovering it should mark it as clickable, and clicking it should download that file from the
  session's host to the user's machine. Which text counts as a path, how relative paths resolve, and what happens for a
  missing file, a directory, or a very large file are to be decided when this is picked up.

- **Copying text from Codex's prompt box does nothing.** Dragging over text typed into Codex's prompt box (the composer
  at the bottom) highlights it, but nothing reaches the clipboard. Reproduced 2026-10-01 against Codex 0.159.3 in a
  plain tmux, outside Farhelm: Codex turns on mouse reporting, so a plain drag goes to Codex rather than to the
  terminal's own selection (SPEC.md, Terminal experience). Over the conversation above the prompt box, Codex copies on
  release by itself (an OSC 52 write, with "Copy sent to terminal" in its footer), and Farhelm forwards that to the
  clipboard. In the prompt box it only highlights; the copy happens only if Ctrl+C is pressed while the highlight is up,
  which Codex does not advertise there. Option-drag on macOS (Shift-drag elsewhere) already works, because it makes
  Farhelm's own selection, which copies on release. So the gap is Codex's behavior, and the fix question is what Farhelm
  should do about it: a hint, making a Codex prompt-box drag copy without the user knowing about Ctrl+C, an upstream
  report, or a combination. Any Codex-specific handling goes where the harness map in
  `crates/farhelm-supervisor/src/agent_kind/mod.rs` says. Not yet checked: whether other harnesses' prompt boxes behave
  the same way.

- **Keyboard quick switcher.** A keyboard shortcut that opens a quick switcher, like Slack's: type to jump to an
  existing session, or to start a new one. Details TBD.

- **Mark a session suspended.** A "mark suspended" action, or similar, so a known, named session can be kept without
  keeping it running. A suspended session is greyed out in the list but can still be reached by name, for example from
  the keyboard quick switcher. The nearest thing today is Stop followed later by Restart, which ends the agent but keeps
  its terminal tabs running and shows the session as exited, the same as any other exit. Whether suspending also stops
  terminal tabs, what bringing a session back does, and how it looks in the list are details TBD.

- **In-app feedback.** A super simple way to give feedback on Farhelm from inside the app. Where the feedback goes and
  what the UI looks like are details TBD.

- **Permission prompts for actions requested through the `farhelm` CLI.** Every action an agent or a user attempts
  through the `farhelm` tool, against any host or session, should ask the user for permission first, with an "always
  allow when coming from this host" option. Model it on the per-host setting for starting YOLO sessions without asking:
  ask by default, and let the user turn asking off for a host. Related to the Maybe later entry on closing the
  cross-host execution hole in agent-requested session creation and cloning.

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
- Tell users that agents and terminal tabs run without the private tmux server's `TMUX` and `TMUX_PANE`, as in an SSH
  login, so shells an agent starts for its own work see no tmux either. A startup file that starts tmux when `TMUX` is
  unset without first checking for an interactive shell (`[[ $- == *i* ]]`) or a terminal can then cut short the
  environment an agent captures from it (Claude Code sources `.zshrc` without a terminal), leaving the agent without
  setup made later in that file, such as a PATH entry; this is the same as running the agent in a terminal outside tmux.
  SPEC.md's "Ownership during cleanup and provisioning" section states the rule.
- Tell users that the sensitive-host YOLO confirmation, and the sidebar's YOLO badge, recognize a custom command line or
  profile as YOLO only on a best-effort basis: common shapes such as a vendor's approval-skipping flag, also behind an
  `env NAME=value` prefix, are caught, but an arbitrary wrapper (a script, `sh -c '…'`) that turns approvals off may not
  be. Launches built with the New dialog's harness and permission choices are always classified exactly. SPEC.md's
  YOLO-launch paragraph will state the rule once `yolo-guard-misses-env-prefix.md`'s triage outcome lands.

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
  repetition `a1b6e9b1-a246-4272-8d7b-89452a3f4c45` passed unchanged. Leading hypothesis (2026-10-01 review, not
  confirmed): the kill landed while `e2e/start-stack.sh` was still in a foreground startup step, and bash defers the
  TERM trap until that command returns, past the check's 20-second wait. One candidate step is the startup session
  create, whose first create probes the systemd user manager under a 15-second bound; nothing yet shows which step it
  was. `start-stack.sh` now prints timestamped lifecycle markers (each startup step, the orphan watcher's decisions,
  when the signal trap actually ran, cleanup stages), and a failed phase dumps the stack's process tree and supervisor
  log tails before its emergency cleanup. On the next failure, compare the watcher's `sent TERM` marker (and the check's
  own timestamped kill of the spawner) with the `signal trap` marker, and look for a long-running foreground command in
  the dump. A missing `sent TERM` line alone is not proof the TERM was not sent: cleanup can kill the watcher between
  its signal and its marker. A cheap check of the mechanism is a PATH `curl` shim that delays the startup
  `POST /api/sessions` by 25 seconds.

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

- **Inject commands into agents.** Let something other than a person typing in the terminal send commands into a running
  agent session. Two motivating uses: a UI button such as "stop all work and restart", and automation, scripts, or an
  orchestrating agent driving workhorse agents that run as Farhelm sessions. The second overlaps with the cross-host
  orchestration authority question in the earlier entry on closing the cross-host execution hole in agent-requested
  session creation.

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
