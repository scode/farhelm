# Helm: delete the unreachable "closed before the handshake" start suggestion

Written against main at aa245430 on 2026-10-24. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Quote the second suggested start command exactly.** When an SSH channel closes before the supervisor handshake, the
> helm suggests a start command with the same double-escaping bug `start-remedy-quoting` fixed in the other one: for a
> path with apostrophes, backslashes and similar characters, it is not exactly the command to paste and can start
> Farhelm on the wrong folder. The plan's report suggests first checking whether real SSH connections can reach it.
> Complexity gate: fix it only if the fix is simple; otherwise leave it. From the review of `start-remedy-quoting` on
> 2026-10-23.

Planning did the check the report asked for. From a read of main at aa245430 (line numbers approximate), a real SSH
connection cannot reach this message, and has not since `fix: surface ssh connection failures (#2505)` (f7f2c187,
shipped in v0.31.0):

- The message is `annotate_ssh_handshake_eof` in `crates/farhelm-helm/src/ssh.rs` (around 478-503). Its only caller is
  the hello-failure arm of the connect phase in `crates/farhelm-helm/src/manager.rs` (around 3819-3831), gated on
  `HostKind::annotates_ssh_handshake_eof` (`crates/farhelm-helm/src/store.rs` around 670-675).
- That call runs only when the attempt carries no SSH process diagnostics (`progress.ssh` is `None`; the branch before
  it, around 3807-3818, returns through `attempt_failure` whenever they are present, and its comment says the fallback
  is for "scripted transports and older callers").
- `progress.ssh` is set once (around 3740) from the transport's connection and never cleared. The production helm uses
  only `SystemTransport` (`crates/farhelm-helm/src/lib.rs` around 2251-2253), whose SSH branch always attaches the
  diagnostics (`crates/farhelm-helm/src/transport.rs` around 151-187 and 482-494). The only transports that attach none
  are test-only scripted ones.
- So a real SSH channel that closes before the handshake goes through the exit-status diagnosis (`ssh_failure_detail`),
  and on exit 75 gets `remote_supervisor_remedy`, the start command `start-remedy-quoting` already fixed (#2567).

The fallback also has two other flaws, both dead in production: it always suggests bare `farhelm`, ignoring the host's
configured program, and it has no length limit, so a long state folder could be cut off by the 1024-byte cap.

The maintainer chose to delete the unreachable fallback.

### Acceptance criteria

- `annotate_ssh_handshake_eof`, its tests in `ssh.rs` (around 1042-1166), `HostKind::annotates_ssh_handshake_eof`, and
  the branch in the connect phase that calls it are removed. A hello failure without SSH diagnostics (only scripted test
  transports produce one) gets the same generic failure context as any other plain transport.
- Anything that existed only for the fallback goes with it, such as helpers or imports left unused. Comments that
  describe the fallback (the "scripted transports and older callers" comment; the `VersionSkew` docs in
  `crates/farhelm-proto/src/io.rs` around 317-321, which say its `Display` text is what the annotator's sibling tests
  assert on; SPEC_impl.md text if any mentions it) are removed or corrected. `ClosedBeforeHello` itself stays: it is
  still used elsewhere.
- Before deleting, confirm by search that no other test asserts the annotation's text ("closed before the handshake",
  "start one there", "farhelm supervisor run" outside `ssh_failure_detail` and its tests), and adjust any that do.
- The guarantee the deleted tests stood for (the "MT-1 regression" tests: a real SSH channel that closes before the
  handshake names the host and the start remedy) stays covered on the live path. Confirm that existing tests show an SSH
  connection through the system transport carries diagnostics, and that an exit-75 close before the handshake gets
  `remote_supervisor_remedy`'s exact-or-descriptive command
  (`start_remedy_is_exact_after_the_final_escaping_or_descriptive`,
  `ssh_natural_exit_categories_preserve_the_remote_evidence` and their neighbours in `manager.rs`). Add one narrow test
  only for a part none of them covers, and log which.
- Not user-visible, so the commit type is `refactor` and there is no changelog fragment (root `AGENTS.md`, "Changelog
  fragments").
- The PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-10-24, on the planning page:

- **Delete the unreachable fallback**, chosen over making it reuse the corrected start command (about 15 lines, but it
  polishes text no user sees) and over dropping the TODO entry with no code change.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry quoted above, and the decisions above.

**Binding repository constraints:** SPEC_impl.md's connection diagnostics paragraphs (around 1755-1786: stderr relay,
failed-attempt diagnostics, exit 75 "supplies the start command, or describes it when the shown command would not be
exact"); root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and
FLAKES.md"; `.agents/test-authoring.md` for the test changes.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): the reachability analysis above.
If you find a production path that does reach the fallback after all, the deletion premise is wrong: block with what you
found, since the maintainer's choice rested on it.

## Outline

One PR, `plan/ssh-close-start-quoting/01-delete-fallback`: the deletion, any coverage gap closed, comment corrections
and the TODO.md removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-helm` (unused code is the main risk), and through `python3 scripts/record-test-run.py` the helm's connection
diagnostics and SSH tests, for example
`cargo nextest run -p farhelm-helm --lib -E 'test(ssh::) | test(start_remedy) | test(ssh_natural_exit) | test(ssh_status)'`.

## Fallbacks

Examples of departures that need the scope reassessment: keeping a reworded fallback, a new transport-level guarantee
mechanism, or changes to `remote_supervisor_remedy` or `ssh_failure_detail`.

Decisions to log: the reachability confirmation you made, every test removed or added and why, and every review finding
you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-ssh-close-start-quoting-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/ssh-close-start-quoting/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
