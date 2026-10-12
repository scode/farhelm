# Execute nine triage outcomes: small robustness fixes in development and release tooling

Written against main at d3b28820 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out nine triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code` with a "fix it only if it
is small" gate, in one draft PR. Their headings, which are also the feedback file names under `review_feedback_queue/`:

- `classification-counts-runner-failures-deterministic-test-failures.md`
- `release-smoke-mistake-another-listener-child.md`
- `replaced-evidence-repeated-continuity-receipts-retain-passing.md`
- `duplicate-scenario-titles-silently-conflate-distinct-sessions.md`
- `preview-ready-identity.md`
- `diagnostic-writers-survive-their-deadline-accumulate-across.md`
- `guest-addresses-account-names-inject-ssh-configuration.md`
- `unbounded-test-discovery-tmux-binaries.md`
- `deflake-duplicates-bounded-evidence-into-unlimited-logs.md`

All nine are in the maintainer's own tooling: the deflake sweep driver, the release workflow's last smoke check, the
release brick test that runs on Mac and Linux VMs, the README hero screenshot staging, the docs preview script, the test
recorder, and one Rust test helper. Nothing changes for someone running Farhelm. Each tool can, under unusual
conditions, give a wrong verdict, misreport success, or use resources without bound:

- A deflake sweep can call a test "fails every time" when its three classification reruns failed for runner reasons (a
  build error, a recorder refusal, a timeout) and the test never ran, sending the agent after a test bug that is really
  an environment problem.
- The release smoke check that proves the published binary serves its web UI only asks "does the port answer HTML". If
  another server grabbed that port, the check could pass while the helm failed to start.
- The brick test validates the bytes of an evidence file and then reads the file again to compute the digest it binds to
  the verdict, so a file replaced in between would carry the passing verdict without having been checked.
- README hero staging keys sessions by title without refusing duplicate titles, so a scenario with two sessions of the
  same title would quietly stage and select the wrong one.
- The docs preview script treats any listener whose working directory is the website directory as its own Astro preview,
  so an unrelated server started there makes it print a link to the wrong service.
- The test recorder's short diagnostic writes spin on a full non-blocking stderr and leave one thread behind per call on
  a blocked one, which matters in the test-hunt loop that calls it in-process up to a thousand times.
- The brick test's guest SSH configuration takes a guest address through `ipaddress`, which keeps an IPv6 scope suffix,
  whitespace and newlines included, and interpolates it into a `HostName` line.
- A supervisor unit test runs `tmux -V` on every `tmux` on PATH with no deadline, so a hanging one stalls the test until
  nextest kills it.
- The deflake driver appends every phase's whole console output to a log with no cap and reads whole logs into memory to
  classify failures.

None of these was observed in practice; all were confirmed by reading the code. Each ledger entry (Assessment, Decision,
Completion criteria) is the authoritative statement of what the PR must do for that outcome. Read the nine entries and
feedback files before starting. Where this file and a ledger entry seem to disagree, the ledger entry wins and the
disagreement is a DECISION to log.

NOT in this plan: `ambiguous-claim-orphaned-next-pick.md`, the tenth tooling outcome from the same decision. Its fix is
an edit to `plans/AGENTS.md`, and a plan's PRs may never touch `plans/` (`plans/AGENTS.md`, States), so it needs a
direct PR outside the planning system. Leave its feedback file, index line and ledger entry alone.

The planning PR that queued this plan set each of the nine outcomes' `TRIAGE_OUTCOMES.md` Execution field to
`` planned in `plans/queue/dev-tooling-robustness-fixes.md` ``. That is the state to expect when you start; this plan's
PR replaces it.

Acceptance criteria:

- One draft PR carries out every outcome whose fix stayed small, each meeting its ledger entry's completion criteria.
  Every outcome whose fix turned out not to be small is recorded as discarded in the same PR, per Small-fix gate below.
- Each changed piece of tooling keeps its existing behavior on the ordinary path; its docstrings or comments say what
  the new guard protects against and why.
- The PR removes every outcome's feedback file and index line (fixed and discarded alike), updates every outcome's
  `TRIAGE_OUTCOMES.md` Execution field, and passed the review gate.
- The checks named under Validation for each touched tool passed.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decision (2026-10-11, recorded in #1919):** asked how strict development and release tooling should
be, the user answered: "two runs in the same checkout is out of scope. its maintainer responsibility not to have
multiple agents in the same checkout. this should be SPEC_impl.md. other than that, fix everything small that's a legit
bug, discard rest". The overlapping-runs outcomes and the SPEC_impl.md sentence belong to another plan. For the nine
here, the decision means: fix it if the fix is small, and otherwise record it as `discard`. Each ledger entry repeats
this as "If the fix is not small, record `discard` instead during execution."

**The user's plan-time decisions (2026-10-11):** "use the planning system to start planning all of the already triaged
entries"; asked how to group them, the user chose "8 plans as proposed", of which this is the development and release
tooling plan; asked about PR shape, "One PR per plan", so this plan's outcomes share one PR, overriding root
`AGENTS.md`'s one-PR-per-outcome rule for these outcomes only; review gate "gpt-6.1-sol high". No-workhorse mode, which
`plans/AGENTS.md` requires.

**Planner proposals** are the fix shapes and the "small" boundaries in the outline below, checked against main by a
fresh-context planning review. They are proposals: a different fix of the same size that meets the ledger's completion
criteria is fine, logged as a DECISION.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Sharing the machine with other agents; Agent scratch space; Claude Code
shell commands; the rule against tests that modify the test process's own environment variables), the root rule that a
change to the deflake driver or its instructions is validated with `deflake/EVAL.md`, `deflake/AGENTS.md` and
`deflake/SPEC.md` for the deflake contract, `docs/test-run-evidence.md` for the recorder, `website/AGENTS.md` and
`website/EDITORIAL_RULES.md` before touching anything under `website/` (a hard requirement there, even for a script),
`releasing/AGENTS.md` and `releasing/mac-vm-test/OPERATIONS.md` for the release tooling, `docs/readme-hero/SPEC.md` for
the hero staging, `plans/AGENTS.md` (Executing one plan), and `review_feedback_queue/AGENTS.md` (Lifecycle).

## Outline

Line numbers drift; find the code by name. Verified on main at d3b28820.

- Deflake verdicts from runner failures (`classification-counts-runner-failures-deterministic-test-failures.md`):
  `deflake/bin/deflake`, `Daemon.classify`, records each rerun as `passed: status == 0` and returns `deterministic` when
  all three reruns failed, whatever made them fail. For a per-test unit (`test.kind` other than `phase`), read each
  rerun's own structured report the way `classify_phase` reads the sweep's (`evidence_dir_from_log` on the rerun log,
  then `nextest/default/junit.xml` through `nextest_failures`, or `playwright.json` through `playwright_failures`), and
  count an attempt as a conclusive failure only when that report names this test as failed. A nonzero exit whose report
  is missing, unreadable, or does not name the test is an inconclusive attempt; record that on the attempt (for example
  `"conclusive": false` with the reason). Verdict: `flake` if any attempt passed, `deterministic` only when every one of
  the three attempts failed conclusively, otherwise `unclassified`. Monolithic `phase` reruns keep their current rule:
  they have no per-test report and SPEC.md for deflake treats the whole battery as the unit. Update
  `deflake/AGENTS.md`'s description of the `unclassified` verdict for a per-test id, which today says it only happens
  when the daemon was stopping, and `deflake/SPEC.md` if its verdict wording needs the same. Add cases to
  `deflake/test-deflake.py` for the new verdict rule with fabricated reports, if the classification can be reached
  without the process-driving path (factor the verdict into a pure function if that is what it takes). Small means: the
  driver's verdict computation, its report readers, one doc paragraph, and unit tests. Not small: changing the event
  schema the agent reads beyond an added per-attempt field, or reworking how reruns are launched.
- Release smoke identity (`release-smoke-mistake-another-listener-child.md`): `.github/workflows/sign-sums.yml`, the
  step that launches `"$binary" helm run --state-dir "$state" --port "$port"` in the background, waits for any answer on
  the port, then requires HTTP 200 and `text/html`. This workflow is hand-written and only called from the generated
  `release.yml` (`publish-jobs = ["./sign-sums"]` in `dist-workspace.toml`), so editing it needs no `dist generate`.
  Check `kill -0 "$helm"` after the wait loop and again after the request (a helm that failed to bind exits, which this
  catches), failing with `helm.log` when either check fails; and require a Farhelm-specific marker in `body.html` in
  addition to the status and content type. Choose the marker from the checked-in UI source (`crates/farhelm-ui`, its
  `Dioxus.toml` and assets, and how `crates/farhelm-helm/src/lib.rs` serves the embedded `index.html`): a string the
  served page is certain to carry and only a Farhelm build would, such as the UI bundle's module name; log which marker
  and why. Do not build a release or a web bundle to find it. Keep the existing diagnostics, which say what a build that
  lost its embedded UI looks like. Small means: this one step. Not small: a new readiness endpoint in the helm.
- Brick test digests (`replaced-evidence-repeated-continuity-receipts-retain-passing.md`):
  `releasing/mac-vm-test/brick_test.py`, `record`, validates
  `contents = [artifact(self.directory, ref) for ref in references]` and later computes `value['sha256']` by calling
  `artifact` again for every bound reference. Hash the `contents` already read for each entry of `references`. The
  continuity receipts (`before`, `after`) are read inside `continuity_chain`; bind their digests to the bytes that
  function validated (have it return them, or read them once in `record` and pass the bytes in), so no reference is read
  twice between validation and digest. Add a case to `releasing/mac-vm-test/test_brick_test.py` that swaps a file's
  bytes between the read and the digest (an injected `artifact` or a monkeypatched read) and shows the digest matches
  the validated bytes. Small means: `record`, at most a signature change of `continuity_chain`, and one test.
- README hero duplicate titles (`duplicate-scenario-titles-silently-conflate-distinct-sessions.md`):
  `e2e/readme-hero/scenario.ts`, the session loop in the validator, checks presence, host, cwd, wrapper, status, age and
  ordering per session, but not that titles are unique, while `e2e/readme-hero/stage.ts` keys its maps by title. Refuse
  a duplicate title with `fail(...)`, in the style of the existing host-key and template-name uniqueness checks in the
  same function. Small means: that one check. Confirm the checked-in scenario (`docs/readme-hero/scenario.json5`) still
  validates. The design files under `docs/readme-hero/` are the maintainer's and change only when asked; do not edit
  them.
- Docs preview identity (`preview-ready-identity.md`): `website/scripts/preview.sh`, `ours_is_serving`, is
  `runs_in "$(listener_pid)" "$website"`. Also require that the listener is this checkout's Astro background server: its
  pid equals `lock_pid "$website/.astro/dev.json"` and `started_before_lock` holds for it, the same evidence
  `stop_server_in` and `take_port` already use for other checkouts. Both the initial check in `start` and the post-start
  check go through `ours_is_serving`, so both get the stronger identity; `status` too. Update the header comment's
  account of how ownership is decided. Small means: `ours_is_serving` and the header. Read `website/AGENTS.md` and
  `website/EDITORIAL_RULES.md` first, as `website/AGENTS.md` demands for any change under `website/`. Do not start a
  preview server from an unattended run except as a short local check you stop afterwards, and never take the port from
  another checkout's server to test this: per `website/AGENTS.md`, an unattended run starts no preview the maintainer
  might be using. If testing it live would disturb a running preview, test the function in isolation instead and log
  that.
- Recorder diagnostic writes (`diagnostic-writers-survive-their-deadline-accumulate-across.md`):
  `scripts/record-test-run.py`, `best_effort_write`, starts a daemon thread per call whose loop retries
  `BlockingIOError` immediately and keeps writing after the caller stopped waiting. Make the writer honor the deadline
  itself (on `BlockingIOError`, wait for writability with `select` for at most the remaining time, and give up when the
  deadline passes) and bound accumulation on a descriptor that blocks forever (for example a module-level record of the
  still-running writer, so a new call while one is stuck drops its diagnostic rather than starting another thread). The
  function's contract stays "best effort, never blocks the caller beyond the timeout". Add cases to
  `scripts/test-record-test-run.py` for a full non-blocking pipe and a never-read blocking pipe. Small means: that
  function and its tests.
- Guest address injection (`guest-addresses-account-names-inject-ssh-configuration.md`):
  `releasing/mac-vm-test/network.py`, `configuration`, normalizes the address with `str(ipaddress.ip_address(address))`
  and interpolates it into `HostName`. Refuse (raise `Refused`, like the username check beside it) an IPv6 address whose
  `scope_id` is set. That is the whole fix: without a scope id, the normalized text of an `ipaddress` object can only
  hold hex digits, `:` and `.`, so no separate character check is needed. Add cases to
  `releasing/mac-vm-test/test_network.py` (a scoped address with whitespace or a newline in its scope is refused; plain
  IPv4 and IPv6 are accepted). Small means: that check and its tests.
- tmux discovery deadline (`unbounded-test-discovery-tmux-binaries.md`): `crates/farhelm-supervisor/src/tmux.rs`, the
  test helper `tmux_binaries_on_path`, runs `std::process::Command::new(&candidate).arg("-V").output()` for each
  candidate. Its only caller, `an_adopted_below_floor_server_is_refused_without_being_killed`, is an async test, so make
  the helper async: run each probe with `tokio::process::Command` and `kill_on_drop(true)` under `tokio::time::timeout`
  with a short deadline (a few seconds), and skip a candidate that times out with an `eprintln!` naming the path
  (dropping the timed-out future kills the child). No polling loop and no `sleep-ok` annotation are needed. The caller
  awaits the helper and otherwise keeps its behavior. Small means: that helper and the one call.
- Deflake log size (`deflake-duplicates-bounded-evidence-into-unlimited-logs.md`): `deflake/bin/deflake`,
  `classify_phase` and `classify` call `log.read_text(...)` on phase and rerun logs that `_run` appends a phase's whole
  output to. Read logs boundedly: a helper that returns the first and last few MiB of a log, from which
  `evidence_dir_from_log` finds the recorder's `test-run evidence:` announcement (printed near the start and, after a
  refusal or error, near the end) and the excerpt takes its tail as now. Capping what `_run` writes would need a copying
  thread between the child and the file and is not small; the ledger accepts the bounded read alone. Add a unit test in
  `deflake/test-deflake.py` for the helper on a log larger than its bound with the announcement at the start. Small
  means: the read helper and its two callers. Fold this together with the verdict fix above, which reads the same logs.
- Changelog and PR type: all nine are maintainer tooling with no effect on someone running Farhelm. Use `chore:` for the
  PR (or another non-user-facing type you can justify; log the DECISION), which adds no changelog fragment per root
  `AGENTS.md` (Releases and the changelog).

### Validation

Follow root `AGENTS.md` "Finishing work": pick the checks that inspect what changed, nothing broader. No Rust battery,
browser, desktop, installer or website build runs: nothing they cover changes, except as named.

- Deflake: `python3 deflake/test-deflake.py`. Root `AGENTS.md` requires the end-to-end procedure in `deflake/EVAL.md`
  for a change to the driver or its instructions, and this plan changes both, so run it once on the final driver. Its
  low-power agent is the subject of the procedure, not a delegate doing this plan's work, so the no-delegation demand
  below does not forbid it; launch it as `deflake/EVAL.md` says. Its fixtures (a once-failing unit test and a wrong JS
  assertion) exercise the flake path and the monolithic path; the new per-test inconclusive path is covered by the unit
  tests. Clean up everything the procedure says to clean up. If the eval cannot run (another sweep holds the machine's
  daemon lock, say), wait for the lock to clear rather than skipping; if it stays held, log it and say so in the report.
- Recorder: `python3 scripts/test-record-test-run.py` (only the cases touching the changed function if the suite offers
  a filter; otherwise the whole file).
- Brick test: `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v`, run through
  `scripts/record-test-run.py` with `--kind development --tmux none`, as `releasing/mac-vm-test/OPERATIONS.md` shows
  (without its operator-specific output root).
- Release smoke: `bash -n` and `shellcheck` on the changed step's script, extracted to a scratch file. No local release
  build or web bundle build.
- Preview script: `sh -n` and `shellcheck` on `website/scripts/preview.sh`.
- README hero: load the scenario through the changed validator once (a one-off `node` or Playwright `--list` against
  `e2e/readme-hero.config.ts`, whichever reaches the validator without staging a fleet), confirming the checked-in
  scenario passes and a copy with a duplicate title is refused. Nothing in CI checks the hero, so this is the evidence.
- tmux helper: `cargo fmt --all -- --check`, `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, and a
  focused recorded nextest run of `test(=tmux::tests::an_adopted_below_floor_server_is_refused_without_being_killed)` in
  `farhelm-supervisor` (check the exact test path with `--list`) through
  `scripts/record-test-run.py --runner nextest --kind development --tmux required`, with the pinned nextest and tmux
  first on PATH per the guarded setup in `docs/test-run-evidence.md`. Read the retained output for runtime `SKIPPED`
  lines. Then `python -B scripts/check-test-sleeps.py` with the isolated interpreter from `docs/test-sleep-check.md`; it
  must report zero unannotated delays.
- `dprint check` on every changed file it covers (`TRIAGE_OUTCOMES.md`, `review_feedback_queue/INDEX.md`, the deflake
  Markdown, and any other changed Markdown or TypeScript).

A failure under another agent's load is an environment event (root `AGENTS.md`, Sharing the machine): keep the retained
run, re-run narrowly, and do not log a flake from one observation.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-dev-tooling-robustness-fixes-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution fields on the PR) rather than starting over. If it does not exist, this is a
fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/AGENTS.md` (Executing one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain. So is the low-power agent that
`deflake/EVAL.md` runs as the subject of its procedure.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/dev-tooling-robustness-fixes/01-tooling-fixes`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect (see the outline's
  changelog bullet). If the change unexpectedly gains a user-visible effect, the type and a fragment under
  `releasing/changelog.d/` follow root `AGENTS.md` (Releases and the changelog), validated with
  `python3 releasing/check-changelog.py format`.
- For every outcome, the PR removes the feedback file under `review_feedback_queue/` and its line in
  `review_feedback_queue/INDEX.md`, and replaces that outcome's `TRIAGE_OUTCOMES.md` Execution field (which reads
  `` planned in `plans/queue/dev-tooling-robustness-fixes.md` ``): `complete` with the jj change ID, bookmark and PR URL
  for a fixed outcome, or per Small-fix gate for a discarded one. Record the change ID and bookmark before creating the
  PR, then add the URL to the same change and push again; no separate bookkeeping PR.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Small-fix gate

The user's rule for these outcomes is "fix everything small that's a legit bug, discard rest". The outline names what
"small" means for each. If an outcome's fix turns out to need more (a new mechanism, a change well beyond the code the
outline names, a design decision, or a change to an interface other tools or agents depend on), do not grow it and do
not block for it: leave the code alone, and in the same PR set that outcome's `TRIAGE_OUTCOMES.md` Execution field to
"`complete`: gate tripped (<reason>); discarded per the recorded decision", followed by the jj change ID, bookmark and
PR URL. Never rewrite the recorded Outcome or mark it superseded: discarding a fix that is not small is what the
recorded decision already says. Remove its feedback file and index line like the others. Name every such discard among
the things the maintainer should know in the plan's report. Code that turned out to be already fixed on main is recorded
the same way, with "already fixed" and the fixing commit as the reason. Block per Unattended fallback only if the shared
PR itself cannot proceed.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria: each outcome's `TRIAGE_OUTCOMES.md` entry, the outline above, and
the user's decisions. The PR changes tests (the deflake, recorder, brick-test and tmux helper tests), so include the
full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Also ask the reviewer to check that
each tool still behaves as before on its ordinary path (a deflake rerun that really failed is still `deterministic`, a
healthy release smoke still passes, a valid IPv4 guest address is accepted, the preview script still recognizes its own
server), and that every discarded outcome's reason is honest. Where you disagree with a finding, decide on the merits
and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new module, a changed
event schema for the deflake agent, a new helm endpoint, edits outside the files the outline names beyond tests and
bookkeeping), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the ledger entries, the user's decisions above, this outline, the
current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled
out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A departure that
the review does not find necessary and small means the outcome is not small, handled by the Small-fix gate.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular how a deflake rerun is judged conclusive and what the attempt record carries, the
bounded log read's sizes, the release smoke's marker string and why only Farhelm carries it, how the brick test binds
continuity receipt digests, how the recorder bounds stuck writers, the guest-address check's exact rule, the tmux probe
deadline, whether the deflake eval ran and how it went, the Conventional Commit type, every outcome discarded by the
Small-fix gate and why, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is the Small-fix gate above. If current code or specs have
moved so that a recorded decision no longer applies, record that outcome as a Small-fix gate trip with the reason, or,
when discarding would plainly go against what the user wanted, set its Execution to `pending`, waiting on the user, with
what changed, keep its queue item, and name it among the report's open questions; never re-triage an item yourself.
Block per `plans/AGENTS.md` (Executing one plan, step 10) only when the shared PR itself cannot proceed without the
user.

You never ask the maintainer anything in conversation while running this plan: not a direct question, not an optional
one, not an offer of more work, and not a turn that ends waiting for a confirmation. Every fork is either decided by you
and logged as a DECISION, or, when only the maintainer can decide it, a block per `plans/AGENTS.md` (Executing one plan,
step 10). Things the maintainer should know and possible follow-ups go into the plan's report, written as statements.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan whose fix stayed small and
records every other one as a Small-fix gate discard, meets the acceptance criteria above, and passed the review gate,
with every outcome's `TRIAGE_OUTCOMES.md` Execution field updated and every outcome's queue item removed. Open, not
merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing
entry in its log, and stop the watchdog. Never edit `plans/` yourself.
