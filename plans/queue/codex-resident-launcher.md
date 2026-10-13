# Capture a Codex session's conversation through a launcher that stays running

Written against main at 25d40da1 on 2026-10-12. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

NOTE: The maintainer met this bug in a private environment. Never write anything specific about that environment into
the repository, a commit message, a PR, a changelog fragment, a report, a log line, a test name, a fixture, or the
working log: no company or product names, host names, user names, paths, version strings or launcher internals. Describe
the case only generically, as "a launcher named `codex` that stays running as Codex's parent". All paths and IDs in
tests are synthetic.

## The goal

The TODO.md entry (Near term), "Support Codex wrappers for the conversation hook": a Codex session launched through a
wrapper loses its conversation hook reports, so Farhelm never learns which conversation the session is in and Restart
cannot resume it.

The case this must cover: the command a session starts resolves to a native launcher executable whose basename is
`codex`. It starts the real native Codex (also basename `codex`, at a different path) as its direct child and stays
running as its parent instead of replacing itself. It forwards Farhelm's injected hook arguments and inherited
environment, and both processes share the session's terminal and foreground process group. The launcher is the session
pane's root process. Every Codex session in such an environment shows the "Codex normally reports its conversation…"
notification, because the supervisor refuses every selecting report with "a nested Codex process cannot report for the
foreground": `codex_corridor` (`crates/farhelm-supervisor/src/procs/codex.rs`) counts both processes as Codex
(`is_codex_image` in `crates/farhelm-supervisor/src/procs.rs`, Linux and macOS) before the pane-root exemption applies.

The fix replaces Codex's structural "exactly one `codex` in the chain" rule with first-report pinning, decided by the
maintainer below.

Acceptance criteria:

- **Pinning.** For a Codex session, the first report of a launch that admission writes (a selecting report it accepts)
  pins the reporting Codex process: the innermost link of the anchored chain whose image is `codex`, by pid and start
  token. The pin also records the `codex`-image links above it in that chain (pid and start token), as its recorded
  Codex ancestors. The pin is written in the same compare-and-swap as that admission; an acked-without-effect `Stop`
  writes nothing and pins nothing. Later reports in the same launch are accepted only when their innermost `codex` link
  is the pinned process, with the two exceptions below. A report from any other Codex process (a nested child, a
  grandchild) is refused, leaving the durable conversation, provenance and offer unchanged; a `Stop` from such a process
  may instead be acked without effect, as `Stop`s that change nothing are today.
- **A pin whose process is gone is released.** When the pinned process is no longer alive (by pid and start token) and
  does not appear in the new report's chain, the pin no longer counts: the report is judged as if the launch had no pin,
  after first checking whether its emitter is a recorded ancestor (which is a correction, below). This keeps working
  what works today: quitting Codex and starting it again from the same terminal, or a launcher that starts a fresh Codex
  child after the first one exits. A descendant of the pinned process whose report arrives after the pinned process died
  stays refused, because the pinned process is in its chain.
- **Any processes above the pinned Codex are accepted**, up to the pane root: further `codex`-named launchers, shells,
  scripts, interpreters, package-manager launchers. The reporter side does not loosen: the reporting process must still
  be the supported hook invocation, and every link between it and the pinned Codex must still be a recognized narrow
  hook trampoline, exactly as today.
- **Self-correction.** When a later report's innermost `codex` link is one of the pin's recorded Codex ancestors, the
  pin was wrong: a nested child reported first. The supervisor moves the pin to that ancestor (whose own recorded
  ancestors are the ones above it) and adopts the conversation it reports as the session's resume target. The ancestor's
  report is verified on its own terms, like a selection, never merged with the current (wrong) binding's file or thread.
  This applies to a `SessionStart` with a foreground source and to a source-less `Stop`, because the parent's next
  end-of-turn `Stop` is what normally exposes the mistake. The pin moves only together with a successful adoption, in
  the same compare-and-swap write: if the ancestor's record fails verification, or an ancestor's `Stop` names a record
  that is not yet resumable, nothing changes (the latter is acked without effect, as today) and the next resumable
  report from it corrects. A correction posts a session notification (the bell) once per launch. Following SPEC.md's
  rule that a notification says what happened and what the user can do or lose, it says in generic words that Farhelm
  had been following a Codex conversation started by another Codex process in this session, that a Restart before now
  would have resumed that one, and that it now follows the conversation of the Codex process that started it. It must
  not claim "the right one": with several nested Codex processes a correction can land on an intermediate one.
- **Lifetime of the pin.** The pin belongs to one launch: it is stored on the session row (so it survives a supervisor
  restart), it is cleared on every relaunch (Restart, Resume, Replace) and not restored by an aborted relaunch, and a
  pin from an earlier launch never applies to a later one. A session with a captured conversation but no pin (an older
  row, or one captured before this change) pins on its next admitted selecting report. Accepted consequence of "first
  report wins": for a session already running across the upgrade, a nested child that reports during its first turn
  after the upgrade can pin itself; the root's end-of-turn report then corrects it and posts the notification.
  SPEC_impl.md says so; build no suppression for it.
- **Everything else holds.** Exact transcript validation, runtime and persistent thread IDs, `/clear` withdrawal and
  pending confirmation, compact, the capture claim and compare-and-swap discipline, generation fencing, quiet and
  bounded hooks, and the existing refusals unrelated to Codex counting all behave as today. Restart of a launcher
  session resumes through the session's configured command, so through the launcher.
- **Ordinary sessions keep working**: a direct `codex`, a shell-started `codex`, `exec codex …` wrappers, and the
  recognized hook trampoline.
- **Docs** describe first-report pinning, the released dead pin, the ancestor correction and the notification:
  - SPEC.md, the Codex reports paragraph in the conversation capture section: "Codex reports must come from the
    foreground native Codex process…, not a nested Codex process that inherited its credential" (it becomes "not a Codex
    process below the pinned one"), and "A later attributed `Stop` … cannot select another conversation or recheck an
    already resumable binding" (an ancestor's `Stop` now selects);
  - SPEC.md's Status paragraph on notifications ("In this version every notification is a session-tracking problem…"),
    which lists every notification kind and says a refused report from a process the session's agent started itself
    never notifies;
  - SPEC_impl.md "Codex attribution and exact-record validation", including dropping "deliberately rejects some
    multi-layer package-manager launchers", scoping "Ready or previously withdrawn bindings return before vendor I/O" to
    the pinned process's `Stop`, and the upgrade consequence above; plus any SPEC_impl.md text that lists the
    notification kinds;
  - the user-facing pages: `website/src/content/docs/docs/agents/codex.md` (the launcher bullet under "Gotchas and
    quirks", which today says a launcher must `exec` codex), `website/src/content/docs/docs/agents/agent-wrappers.md`
    (the page about wrappers that stay running as the agent's parent, which has only Claude's rule; add Codex's), and
    `website/src/content/docs/docs/how-it-works/agent-hook-injection.md` (its Codex attribution and `Stop` sentences,
    including "A nested persistent or ephemeral invocation cannot replace its parent's target").
- **Tests** as listed under Outline, including a real end-to-end run through a resident launcher fixture.
- A linear stack of draft PRs, each passing its review gate, with a changelog fragment, the last one removing the
  TODO.md entry. No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-12):** plan this TODO entry. The maintainer supplied a diagnosis from the affected
environment, summarized generically in The goal above; this file is its only record.

**The user's decisions (2026-10-12):**

- Fix it on Farhelm's side; the launcher is not at fault and does not need to change.
- The maintainer's idea, in their words: "accept the first report on the assumption that sub-agents would never report
  back until after the parent reports in first", and "at that point, we identify the process." That is the pinning
  above.
- Then: "if in the future we get a report from the parent of that process, we can send a notification telling the user
  that we detected a bug in this environment and that the previous conversation was likely wrong." Offered the options
  of correcting and notifying, notifying only, or correcting silently, the maintainer chose **correct and notify**.
- Scope: pinning applies to **every Codex session**, with any processes above the pinned Codex accepted, rather than
  only to chains with two `codex` processes. The maintainer's reasoning: one rule instead of several special cases, and
  accepting any number of levels is a minor difference. The tradeoffs were stated and accepted: ordinary sessions trade
  today's outright refusal of a nested child's report for "first report wins, the parent's next report corrects it";
  launchers that SPEC_impl.md deliberately started refusing are accepted again; Codex attribution becomes unlike
  Claude's, Grok's and OMP's, which stay as they are.
- A pin whose process has exited no longer counts, so a new Codex in the same launch can pin afresh (offered and not
  chosen: keep the pin for the whole launch and document that a second Codex is refused until Restart).
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.
- Never write non-generic details about the affected environment anywhere (the NOTE at the top).

**Binding repository constraints:**

- SPEC.md and SPEC_impl.md as named above; this plan changes them as decided, in the same PRs as the code.
- Root `AGENTS.md` "Harness-specific code" and the module docs of `crates/farhelm-supervisor/src/agent_kind/mod.rs`:
  per-harness behavior lives in the places that map names (attribution in `procs/codex.rs`, stateful admission in
  `service/core/vendor/codex.rs`, per-kind helpers in `agent_kind/codex.rs`); no `kind == X`, `matches!` over harnesses
  or `_` arm over harnesses in shared code.
- SPEC.md "Upgrade compatibility and client scale": the upgrade path from v0.23.0 must keep working; a new durable
  column needs a migration and a defined default for existing rows, and must not break reading the stored conversation
  record (the `codex:` locator is `deny_unknown_fields`, so the pin must not go inside it).
- Root `AGENTS.md`: Conventional Commits; Finishing work (Rust checks through the recorder,
  `scripts/check-test-sleeps.py` for test delays); Releases and the changelog; Agent scratch space; Sharing the machine
  with other agents; Docs website (read `website/AGENTS.md` and `website/EDITORIAL_RULES.md` before touching any page).
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the mechanism, stack and test shapes in the outline below, checked against main and corrected
by a fresh-context planning review.

## Outline

Line numbers drift; find the code by name.

Here's what exists today:

- `codex_corridor(chain: &[ChainLink]) -> Result<ProcessIdentity, String>` in
  `crates/farhelm-supervisor/src/procs/codex.rs` takes the anchored chain, reporter first and pane root last. It
  requires the reporter to be the hook invocation, refuses a second `codex` image, and requires every link other than
  the first and the last to be a hook trampoline (refusing other session-hosting runtimes and unclassified
  intermediaries), including links above the Codex image. `ProcessIdentity { pid, start }` is in `procs.rs`.
- Its only production caller is `report_codex_conversation` in
  `crates/farhelm-supervisor/src/service/core/vendor/codex.rs`, which uses the returned emitter only in a log line.
  Nothing stores a reporting process for any harness today. The function's enrichment (`Stop`) early returns (no
  parseable captured locator, ownership version, resumable, thread set, runtime differs) run before the corridor, so a
  `Stop` from any process is acked without effect.
- Durable per-session state on the `sessions` row (`crates/farhelm-supervisor/src/store.rs`): `captured_conversation`
  (`codex:` + JSON `CodexLocator`, `deny_unknown_fields`), `capture_ownership_version`, `conversation_source`,
  `generation`. Writers are compare-and-swap (`admit_ownership_proven_conversation`,
  `replace_reported_conversation_if_current`) under `claim_capture_for_report`. The precedent for per-launch columns is
  OMP's `omp_reporter_asset`/`omp_launch_program` (`record_omp_launch_provenance`), cleared in `begin_relaunch`.
- Hook reports spool to `<state>/hook-reports/<id>/selection.json` and `enrichment.json` (`hook_report.rs`,
  `service/report_files.rs`); each slot keeps only the latest report, and selection drains before enrichment. During an
  outage before anything is pinned, a child's and its parent's selections can collapse into whichever was written last;
  the ancestor correction is what recovers from the child winning that way. Once the root is pinned, the durable pin
  refuses the child outright.
- Session notifications: `Supervisor::notify_session(id, generation, NotificationKind, text)` in
  `service/notifications.rs`; `NotificationKind` (`HookSilent`, `HookNotAdded`, `ResumeWithdrawn`, `ReporterMismatch`)
  has exhaustive `ALL`, `as_str`, `resolves_on_resume`, `requires_no_identity`; one row per (session, generation, kind),
  at most ten per session; a kind that never resolves posts at most once per launch with no new logic, because the store
  does nothing while an unresolved row for that key exists. OMP's `ReporterMismatch` in `service/core/vendor/omp.rs` is
  the nearest admission-time example.
- Tests: the corridor unit tests in `procs.rs` (for example
  `a_wrapper_above_the_runtime_is_the_pane_anchor_not_an_intermediary`,
  `a_package_wrapper_below_a_separate_pane_anchor_is_refused`, `a_second_codex_image_still_refuses`,
  `another_runtime_between_reporter_and_foreground_is_refused`) with `corridor_link` and `hook_argv` helpers; the e2e
  module `crates/farhelm/tests/e2e/codex_identity.rs` (`a_shell_child_cannot_claim_a_pristine_codex_session`,
  `nested_native_codex_reports_cannot_replace_the_foreground_conversation`, helpers `native_codex_image`,
  `durable_binding`, `wait_for_offer`, `supervisor_verdict`); the fake agent fixture
  `crates/farhelm-fixtures/src/fake_agent/codex_conversation.rs` (`--record-home`, `--hook-binary`, `--codex-child`,
  `--resume-id`, `--defer-startup`; pane commands such as `report-startup`, `nested-persisted`,
  `nested-shell-persisted`, `clear-pending`, `stop`, `stop-other`, `compact`, `recall`). `native_codex_image` hard-links
  (or copies) the fixture binary to a file named `codex`, because a symlink would resolve to the fixture's own name.

Here's the work, as a proposal. Shape the stack as you see fit, but every PR must leave main correct on its own.

1. **Corridor.** Change `codex_corridor` to return the innermost `codex` link as the emitter plus the `codex` links
   above it, requiring only the reporter and the reporter-to-emitter trampolines as today, and accepting anything above
   the emitter. Keep "a chain without any codex image is refused" and the reporter-side refusals. Per the harness map,
   the pin decision itself belongs in the stateful Codex admission, not in shared code.
2. **The pin.** One nullable TEXT column on the session row (JSON of the pinned pid and start token plus the recorded
   ancestors; NULL means no pin), not inside the locator; existing rows default to NULL with no backfill. Written in the
   same compare-and-swap as the admission it comes from. Admission re-reads the row under `claim_capture_for_report` on
   every report, so no in-memory mirror or reload step is needed. Either clear it in `begin_relaunch` beside the OMP
   columns or stamp it with its launch generation and treat a mismatch as no pin; either way `abort_relaunch` must not
   restore it.
3. **Admission order.** In `report_codex_conversation`, run the corridor and the pin check before the `Stop` early
   returns, so that a `Stop` from a recorded ancestor can select. Rules, in order: a recorded ancestor (`SessionStart`
   with a foreground source, or source-less `Stop`) → verify its exact record on its own terms, skipping the enrichment
   branch's merge of the current binding's file and thread, then re-pin, adopt and notify in one write (a not-yet-
   resumable ancestor `Stop` is acked without effect by the existing early return); no pin, or a pin whose process is
   gone and absent from this chain → judge as today and pin on admission; the pinned emitter → as today, its `Stop`
   keeping today's confirmation-only rules; anything else → refuse with a clear reason, or ack a `Stop` without effect.
4. **Notification.** A new `NotificationKind` with the text described in the acceptance criteria. Its flags are forced,
   not chosen: `requires_no_identity` false (it is posted right after a capture, and the store refuses the insert
   otherwise) and `resolves_on_resume` false (otherwise the capture pass would resolve it at once).
5. **SPEC.md, SPEC_impl.md and the two website pages** as in the acceptance criteria.
6. **Tests.**
   - Corridor unit tests in `procs.rs`: the launcher shape (pane-root `codex` launcher → `codex` → hook, with and
     without the trampoline) yields the inner Codex as emitter with the launcher recorded as its ancestor; arbitrary
     intermediaries above the emitter are accepted; reporter-side refusals unchanged. Rewrite rather than delete
     `a_second_codex_image_still_refuses` and the package-wrapper test, so they state the new rule.
   - Pin and correction tests at the admission layer, against the real store, with synthetic anchored chains as the
     corridor tests use (`report_codex_conversation` takes an already-anchored chain): first selecting report pins; a
     non-ancestor `codex` report is refused with the binding byte-identical; an ancestor's `SessionStart` and an
     ancestor's `Stop` each re-pin, adopt and notify once; an ancestor's record that fails verification moves nothing; a
     dead pin is released (a liveness seam is needed for this one) while a dead pin's descendant stays refused; the pin
     survives a supervisor restart; Restart clears the pin and the new launch's first selecting report pins its own
     process.
   - End to end in `codex_identity.rs`: a resident launcher fixture, a native executable named `codex` (hard link or
     copy, as `native_codex_image` does) that spawns the distinct Codex fixture image as its direct child, forwards all
     arguments and environment, waits for it and exits with its status, without exec'ing itself away or creating a new
     process group. Prefer a fixture mode of the existing fake agent binary over new compiled code. Declare the session
     through the launcher, with a resume command through it too, and keep the launcher as the pane root. Prove: real
     supervisor admission with the inner Codex as emitter, the durable locator and provenance, the public Resume offer,
     Restart through the launcher resuming the right conversation, and a nested child's later report leaving all of that
     unchanged.
   - A new e2e for the child-first case, from existing fixture commands: `--defer-startup`, then `nested-persisted` (a
     native `codex` child running its hook directly, which pins), then the root's `report-startup` (an ancestor
     `SessionStart`) or the root's `stop` (an ancestor `Stop`; the root's record already exists under
     `--defer-startup`). Prove the correction, the notification, and that Restart resumes the root's conversation. Real
     Codex fires `SessionStart` at the first prompt, before any sub-agent can start, so in practice a child reports
     first only through an outage or across the upgrade; the test has to construct it.
   - Leave `a_shell_child_cannot_claim_a_pristine_codex_session` as it is: its child is a plain shell running the hook,
     not a second `codex`, so the unchanged reporter-side rule still refuses it, and it guards that rule. Likewise the
     `nested-shell-persisted` and `nested-shell-fileless` cases in
     `nested_native_codex_reports_cannot_replace_the_foreground_conversation`; only `nested-shell-native` changes its
     refusal reason, from the Codex count to the pin.
7. **Bookkeeping.** Changelog fragment, `kind: fixed` (Codex sessions started through a launcher that stays running now
   capture their conversation and can be restarted), written per `releasing/EDITORIAL_GUIDANCE.md` and generic. Remove
   the TODO.md entry.

The fix cannot be confirmed in the environment where the bug was seen. Say so in the report and ask the maintainer to
try a Codex session there once it ships: capture, a `/clear`, and Restart.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`;
- through the recorder with the pinned nextest and tmux (`docs/test-run-evidence.md`): the supervisor's `procs::` and
  Codex admission tests, the store tests the migration touches, and the `farhelm` e2e `codex_identity::` module, then
  whatever the diff shows the change can reach (other `conversation_identity`/`hook_identity` e2e modules, notification
  tests);
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
- `dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`, and the website build
  (`cd website && bun install --frozen-lockfile && bun run build`) for the two pages.

Do not run the full battery without a specific risk that calls for it.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-codex-resident-launcher-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout. The NOTE at the top applies to the working log as much as to anything
committed.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/codex-resident-launcher/<nn>-<short-name>` (for example `plan/codex-resident-launcher/01-pin-first-report`).
- Bite-sized PRs, each leaving main correct on its own, and no churn: code added in one PR and deleted in a later one
  means the stack should have been shaped differently. Within this run, a PR that needs correcting is restructured
  rather than corrected on top, and that applies to PRs an earlier run built too. The corridor relaxation, the pin and
  the ancestor correction land in one PR: the relaxed corridor without the pin would let every nested child replace the
  root's conversation, and the pin without the correction would leave the child-first case with nothing to close it. An
  inert storage PR before it (the new column, unused) is fine.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect (`fix`). A `fix`
  PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the
  changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything. Keep every one of them generic (the NOTE at the top).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

The PRs change tests, so include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check:

- that no report other than the pinned process's or a recorded ancestor's can change the session's conversation, and
  that a refusal leaves the durable binding byte-identical;
- that the pin is written in the same transaction as its admission, moves only with a successful adoption, is cleared on
  relaunch and not restored by an aborted relaunch, and that a released dead pin can never let a descendant of the
  pinned process report;
- that the reporter side (hook invocation and trampolines) is as strict as before;
- that the per-harness rules in `agent_kind/mod.rs` are followed;
- that nothing environment-specific appears anywhere.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example changing the hook report spool into a
  queue, adding a launcher-recognition list or a user-declared launcher setting, changing another harness's attribution,
  or a protocol change (examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered. In particular:

- how the pin is stored, migrated and cleared;
- the new admission order in `report_codex_conversation`;
- how the dead-pin liveness check and its test seam work;
- the notification's exact text;
- how the launcher fixture is built and declared;
- the stack's shape;
- the SPEC.md, SPEC_impl.md and website wording;
- every review finding you decided not to follow.

The user will ask for these later. Keep the log as generic as everything else.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: keeping the structural refusal for ordinary
sessions, narrowing pinning to two-`codex` chains, dropping the correction or the notification, changing Claude's,
Grok's or OMP's attribution, or loosening the reporter side.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
