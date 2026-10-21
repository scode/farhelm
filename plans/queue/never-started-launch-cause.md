# Say which step failed when an agent never started, and keep what it printed

Written against main at 2897c914 on 2026-10-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file says which of their sentences to change.

This plan has no dependency on another plan.

## The goal

"Never started" means a single launch that failed on the spot: the user presses Launch or Restart, Farhelm creates the
session and its terminal, and the agent process is never created at all. It is not about reboots or supervisor restarts;
a session whose agent was lost to a host reboot is **interrupted**, which this plan does not touch.

On a Linux host with a systemd user manager, starting an agent is a short relay inside the new terminal: the user's
login shell runs its startup files, hands over to a systemd scope wrapper (`systemd-run --scope`, which gives the agent
its own cgroup so Stop can end it cleanly), which hands over to Farhelm's launch shim (`farhelm internal launch`), which
starts the agent. When the relay breaks before the shim runs, the session shows **error** with "the agent was never
started: the launch never reached farhelm's exec shim, so something before it — the transient cgroup scope wrapper, or
the login shell itself — exited first". That names two suspects and leaves the user guessing: a `.bashrc` or `.zshrc`
that exits or replaces the shell needs a fix in the user's own files, while a refused scope is a host problem.

Make the error name the one step that failed, with the exit status, and keep a short excerpt of what the failed terminal
printed with the error, so the user sees the shell's or wrapper's own message even after the terminal is gone. The user
chose to weaken SPEC.md's terminal-retention promise for this, in their words: "we avoid storing terminal output
anywhere, except when it's directly useful for a sensible user experience such as error reporting". Also give hosts
without systemd scopes (macOS, Linux without a systemd user session) the same **error** for a login shell that died
before Farhelm's launch command, instead of today's plain **exited**.

TODO.md entry this plan covers (Near term): "Make the never-started verdict say which link died. When a scoped launch
dies before farhelm's exec shim, the supervisor's `wrapper_failure_detail` (launch_artifacts.rs) records "the agent was
never started: the launch never reached farhelm's exec shim, so something before it — the transient cgroup scope
wrapper, or the login shell itself — exited first", which names two suspects and separates neither. The wrapper's stderr
is still sitting in the dead pane under `remain-on-exit`, so a `capture-pane` at classification time could say which. A
first attempt (2026-08-23) appended the pane's last words to the durable `LastOutcome::Error` detail and was withdrawn
in review for three reasons any retry must design around: (1) SPEC.md's terminal-retention contract — terminal content
lives only as long as the host-side terminal, with no separate history store — which a durable excerpt of startup/rc
output contradicts, so either the quote must not be persisted (log it, or surface it only while the pane exists) or the
spec must authorize a bounded exception first; (2) the pane is reused across relaunches and keeps its scrollback, so a
generation N+1 that died before printing anything would quote generation N's conversation unless the capture is fenced
to text written after the wrapper started; (3) the ownership-and-deadness check and the capture are separate steps with
no lifecycle claim across them on the list path, so a same-pane restart in between would quote a live later generation —
revalidate atomically with the capture, and budget the capture so N never-started rows cannot cost N tmux timeouts on
the hot list. The e2e harness already has `wait_for_agent_ready` (harness.rs), whose failure text shows the same pane
text for a test's own diagnosis; that is the non-durable shape to start from."

Reason (1) is now settled by the user's decision to change the spec. Reasons (2) and (3) still bind, as acceptance
criteria below. Reason (3)'s "hot list" has moved: since a change on 2026-10-09 ("make session refreshes read supervisor
state"), session lists no longer classify; the supervisor's background observer and its startup reconciliation do. The
budget concern now applies to the observer's pass.

### Acceptance criteria

**Telling the two suspects apart (scoped launches):**

- The scoped window command becomes a single `exec` of a new Farhelm step (name is yours, for example
  `farhelm internal launch-wrapped <spec> -- <systemd-run …>`). That step creates an empty, generation-named waypoint
  file beside the launch spec (inside the existing 0700 launch directory, created without following symlinks, like the
  spec and the sentinel), then execs the scope wrapper with the ordinary `<farhelm> internal launch <spec>` appended. If
  the exec of the wrapper itself fails, it writes the ordinary sentinel naming the wrapper and the errno, which the
  existing sentinel path already turns into **error**. The `-c` script stays one `exec`, so
  `window_command_script_runs_nothing_before_the_exec` and SPEC.md's no-preamble rule hold unchanged; the process tree
  after the execs is unchanged. Prior art for a Farhelm step that execs onward: `tab_window_command` and
  `exec_tab_shell` in `crates/farhelm-supervisor/src/launch.rs`.
- Classification uses the waypoint and the dead pane's exit status (`PaneState::exit_code` in
  `crates/farhelm-supervisor/src/tmux.rs`, known at classification time and unused today). No waypoint: the login shell
  never ran Farhelm's launch command. Waypoint present: the shell got there and the scope wrapper failed.
- Upgrade safety: a scoped launch started by an older build has no waypoint step and must not be blamed on the login
  shell. The launch spec (or the stored launch record) carries a "waypoint expected" flag; without it the classifier
  keeps today's two-suspect sentence.
- The waypoint file joins the launch artifacts: the startup sweep (`parse_launch_file_name` accepts only `.json` and
  `.status` today), the superseded-launch sweep and post-classification cleanup handle it like the spec and the
  sentinel.

**Hosts without systemd scopes:**

- An unscoped launch with the same evidence shape (dead pane, launch spec still unconsumed, no sentinel) becomes
  **error** with the login-shell wording, instead of today's plain exit. In an unscoped launch the shim is already the
  first Farhelm step, so that shape already proves the shell never reached it; launches started by older builds have the
  same shape, so no compatibility flag is needed there. The deliberate scoped-only narrowing in
  `wrapper_failure_detail`'s docs is rewritten.
- Neither change may turn a user's Stop during shell startup, a reboot (which is **interrupted**), or any launch that
  reached the shim into **error**. Keep the existing "unattributed" and stop handling in the observer (`observe_entry`
  in `crates/farhelm-supervisor/src/service/status.rs`) and in startup reconciliation
  (`crates/farhelm-supervisor/src/service/core.rs`, the `wrapper_failure_detail` call in `reload_sessions`) intact.

**Keeping an excerpt of what the terminal printed:**

- When a launch is classified as never started (either suspect, scoped or not), the supervisor captures a bounded
  excerpt of the dead pane's last lines and appends it to the durable error's detail (`LastOutcome::Error { detail }`),
  after Farhelm's own wording. Bounds are yours within "a few lines": for example at most 5 lines and 500 bytes, taken
  from the end, with control characters and escape sequences stripped or escaped the way other peer text is. Log the
  bounds.
- **Fenced to this launch.** The pane is reused across relaunches and keeps its scrollback (`respawn-pane` keeps
  history, per SPEC_impl.md), so the capture must include only text written after this launch generation's command
  started. If the fence cannot be established, store no excerpt; never risk quoting an earlier generation's
  conversation. A supervisor restart between the launch and its classification may lose the fence; that costs only the
  excerpt.
- **Atomic with the deadness check.** The capture and the check that the pane is still this generation's dead pane
  happen in one tmux invocation, or with a re-check in the same invocation, so a same-pane restart racing the
  classification is never quoted. `Tmux::capture_pane_text` (which quotes a dead pane's last words for a refused tab
  open) is the nearest existing capture, but it does no fencing and no revalidation.
- **No extra cost on the observer.** Each capture uses the tmux driver's existing timeout. After one capture times out,
  the rest of that pass store their errors without an excerpt rather than holding up the pass.
- The badge's visible start stays Farhelm's own wording, because the excerpt comes last; the badge already carries its
  full text in its tooltip (`StatusBadgeView` in `crates/farhelm-ui/src/status.rs`), which is where the excerpt is read.
  A sibling store field or a new UI surface is allowed only if the tooltip demonstrably cannot render the excerpt
  usefully, logged as a DECISION.

**Wording (yours to finalize; log it):**

- Login shell: "the agent was never started: your login shell exited (code N) before it ran Farhelm's launch command; a
  startup file such as .bashrc or .zshrc usually exits or replaces the shell", followed by the excerpt when there is
  one. "code" matches both the user's pick and the existing **exited (code N)** badge.
- Scope wrapper: "the agent was never started: the systemd scope wrapper exited (code N) before starting the agent; the
  host's systemd user manager may have refused the scope", followed by the excerpt.
- The code phrase is omitted when tmux reports none (a signal death). Keep it short: the badge shows the start of it.

**Specs and docs:**

- SPEC.md: rewrite the terminal-retention promise around SPEC.md:44 ("terminal contents live only as long as the
  host-side terminal does") and in Terminal experience around SPEC.md:1731-1735 ("There is no separate history store")
  to the user's principle: Farhelm avoids storing terminal output anywhere, except where it is directly useful for a
  sensible user experience such as error reporting; name the never-started excerpt as such a case and its bounds. Keep
  the separate no-snapshot rule (no captured frame of a full-screen program is retained or painted on restart, SPEC.md
  and SPEC_impl.md explain why a painted frame with no process behind it is worse than a blank pane): it is about
  restarts, not error reporting, and the user did not change it.
- SPEC.md's **error** definition and the launch-failure paragraph stay correct; check them against the unscoped change.
- SPEC_impl.md: the launch sentinel contract around SPEC_impl.md:1552-1572 (the waypoint step, the new evidence shapes,
  the two wordings, the excerpt with its fence and atomicity), the scope paragraph that says where the wrapper sits in
  the chain (the `window_command` docs in `launch.rs` mirror it), and the launch-artifact list around
  SPEC_impl.md:3030-3036.
- A changelog fragment (`kind: changed` or `fixed`; log which) saying what a user now sees, including that hosts without
  systemd scopes now show error for this case.
- The last code PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-20/21), from the planning page:**

- How to tell the suspects apart: "A Farhelm step at the head of the chain writes a per-launch marker file", chosen over
  a shell-side marker, exit status alone, and quoting the terminal alone.
- Terminal text: "Save the last few lines with the error permanently, as an exception to the promise", chosen over no
  copy and over an in-memory-only excerpt. The user added: "also we should just weaken the promise in general. we avoid
  storing terminal output anywhere, except when it's directly useful for a sensible user experience such as error
  reporting".
- Hosts without scopes: "Yes: show "error — the agent was never started: your login shell exited (code 1) before it ran
  Farhelm's launch command" there too", chosen over keeping **exited**.
- The user asked whether "never started" was about an agent lost to a supervisor or host restart. The planner explained
  that it means a single launch failing on the spot (as The goal above says); the user's later answers were given on
  that basis.
- Review gate and mode are in the run instructions below.

**Binding repository constraints:**

- SPEC.md's no-preamble rule for new processes (nothing painted ahead of the exec) and its no-snapshot rule; SPEC.md's
  status definitions (**error**: "the agent process could not be started at all"; **exited** with its code;
  **interrupted** after a reboot).
- SPEC_impl.md's sentinel contract and its NOTE that a shell-side sentinel after `exec` was audited and rejected (zsh
  terminates on a failed exec). The waypoint step is a Farhelm program, not shell code, for that reason.
- The launch artifacts' generation naming (`spec_path_for_launch`, `status_path_for_spec` in `launch.rs`), so an older
  launch's files can never describe a newer one.
- Root `AGENTS.md`, "Code rules", "Changelog fragments" and "Tests and validation"; `.agents/test-authoring.md`.

**Planner proposals (checked by a fresh-context planning review):** the outline below, including the waypoint's name and
file shape, the compatibility flag, and the fence mechanism. A planner proposal is not a requirement: a smaller design
that meets the same acceptance criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main (verify them; line numbers drift):

- `wrapper_failure_detail` (`crates/farhelm-supervisor/src/service/launch_artifacts.rs`, around 57) returns the quoted
  two-suspect string. Its inputs are only "scoped", "pane dead" and "spec still on disk"; its docs explain the
  scoped-only narrowing. Its shape test is `a_wrapper_failure_is_recognized_only_by_its_full_shape`.
- Callers: the observer's `observe_entry` (`service/status.rs`, around 651, call around 720), reached from the ticker
  and the pane-death wake, and startup reconciliation in `service/core.rs` (call around 6161). List replies never
  classify.
- The window command (`window_command` in `launch.rs`, around 631) is
  `<login shell> -l -i -c "exec [<systemd-run …>] <farhelm> internal launch <spec>"`. The shim reads and unlinks the
  spec as soon as it starts and writes a sentinel on every failure after that (`exec_launch_spec_with_seam`). Pins:
  `window_command_script_runs_nothing_before_the_exec` and `a_scope_prefix_wraps_only_the_shim_invocation`.
- The in-memory, non-durable launch-error cache (`RunCells::launch_error` in `core.rs`) is filled by the observer and
  preferred by the listing until a durable outcome lands; the durable outcome is `LastOutcome::Error { detail }` in
  `crates/farhelm-supervisor/src/store.rs`.
- The UI renders the badge as "error — <detail>" (`crates/farhelm-ui/src/status.rs`) and the session view's restart
  offer as "the agent never started — …" (`crates/farhelm-ui/src/session_view.rs`).
- E2E: `a_failed_scope_wrapper_classifies_as_error_rather_than_a_plain_exit`
  (`crates/farhelm/tests/e2e/session_lifecycle.rs`) plants the evidence shape on a real scoped launch, skips without a
  systemd user manager, and asserts the old wording; `wait_for_agent_ready` in `crates/farhelm/tests/e2e/harness.rs`
  shows the pane text for a test's own diagnosis.

Planner proposals for the mechanisms the acceptance criteria leave open:

- The fence: read the pane's history position (for example tmux's `history_size` plus the cursor row) just **before**
  `respawn-pane`, while the dead pane is stable (afterwards the new process may already have printed), and keep it in
  the launch's in-memory run state for that generation, next to `RunCells::launch_error`, not as a new launch artifact.
  A launch in a new window needs no recorded position. Capture only from that position, treating any position the
  history limit has since trimmed past as nothing reliable to quote.
- Optional, not a user requirement: a vanished Farhelm binary leaves no waypoint and reads as "login shell". If cheap,
  check the Farhelm path the launch itself used (the launch spec can carry it beside the "waypoint expected" flag),
  never the current supervisor's own path, which may differ after an upgrade. If it costs more than that field, drop it
  and accept the login-shell wording with its code, logged.

Proposed shape (planner proposal), as a short linear stack:

1. `fix`: say whether the login shell or the scope wrapper stopped a launch. The waypoint step, window command,
   classifier with exit code, compatibility flag, artifact sweep and cleanup, the unscoped reclassification, unit tests
   and the e2e update, SPEC_impl.md, a changelog fragment.
2. `feat` or `fix`: keep a bounded excerpt of a never-started launch's terminal output. The fenced, revalidated capture,
   the excerpt appended to the stored detail, the SPEC.md promise rewrite, tests, the fragment (editing PR 1's if both
   are unreleased), and removal of the TODO.md entry.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the supervisor's unit tests for `launch.rs`, `launch_artifacts.rs`,
`status.rs` and `tmux.rs`, and the affected e2e tests in `session_lifecycle.rs` through
`python3 scripts/record-test-run.py` (scope-gated tests need a systemd user manager; read the retained output, since a
skip is a JUnit pass); the UI crate's unit tests if the UI changes; `python -B scripts/check-test-sleeps.py` for changed
tests; `python3 releasing/check-changelog.py format`.

New tests the outline implies: the classifier picks the login-shell wording without a waypoint, the wrapper wording with
one, the old wording when no waypoint was expected, and includes the exit code when present; the scoped window command
is still a single `exec` and starts with the waypoint step; the waypoint step writes its generation-named file and execs
the given argv, and an unexecutable wrapper path produces a sentinel naming it; the sweep recognizes, keeps and removes
waypoint files correctly; an unscoped launch whose shell died first classifies as error, and a Stop during startup does
not; the excerpt contains only text written after this generation's launch (plant earlier scrollback in a reused pane
and prove it is not quoted), is bounded and escaped, is absent when the fence cannot be established, and is never taken
from a pane that was restarted between the check and the capture; a real reproduction of the login-shell case if the e2e
harness can give the session a startup file that exits (check whether it controls the session's HOME or shell before
relying on it).

## Fallbacks

If an atomic capture-plus-revalidation is not achievable with tmux on some supported version, store the error without an
excerpt there rather than relaxing the check; log it and say so in the report. If the fence cannot be made reliable at
all, ship PR 1, store no excerpt, and block with the concrete tradeoff rather than quoting unfenced text.

Examples of departures that need the scope reassessment: a second command in the window command's `-c` script; a general
terminal-output store beyond this error excerpt; capturing on the list path; changing how reboots or stops are
classified.

Decisions to log: the waypoint step's name and file suffix, the compatibility flag's home, the excerpt bounds and
sanitizing, the fence mechanism, whether the missing-binary check was kept, the final wordings, and the fragment's kind.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-never-started-launch-cause-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user explicitly chose no-workhorse mode for this plan. The user forbids sending any unit of your own decomposition
of this work to a weaker model or a lower effort than your own. The review gate and the scope reassessment review are
not units of that decomposition: they keep the model and effort this file demands for them. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The scope reassessment review runs
at your own model and effort. The sub-agents `plans/AGENTS.md` (Sub-agents this file requires) requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
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

- Split the work into a linear stack of reviewable PRs, one reviewable commit, stable bookmark and draft PR each, using
  the `jjstack` skill. The outline above suggests the slicing; err on the side of bite-sized PRs.
- The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/never-started-launch-cause/<nn>-<short-name>` (for example `plan/never-started-launch-cause/01-waypoint`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

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

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
