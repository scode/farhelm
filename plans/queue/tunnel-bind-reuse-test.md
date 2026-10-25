# Test: localhost tunnel listeners bind without address reuse

Written against main at 36c8d7b0 on 2026-10-24. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Test that localhost tunnels bind without address reuse.** Mac localhost tunnels bind their listener with address
> reuse off, so a tunnel cannot steal another program's connections; nothing tests that, and on Linux every test would
> still pass if it regressed. Add a test. From the review of `tunnel-port-and-link-ranges` on 2026-10-23.

What the code does today, from a read of main at 36c8d7b0 (line numbers approximate):

- `bind_listener` in `crates/farhelm-helm/src/tunnels.rs` (around 229-258, docstring included) builds the listener from
  tokio's `TcpSocket`: `new_v4()`, `set_reuseaddr(false)`, bind `127.0.0.1:0`, `listen`. If the kernel hands back the
  remote port number by chance, it binds again while holding the first socket. Its only caller is `open_from` (around
  393). Nothing is cfg-gated; the same code runs on Linux, where the tests run.
- The shape exists on purpose: tokio's `TcpListener::bind` (through mio) and std's `TcpListener::bind` both turn
  `SO_REUSEADDR` on outside Windows. The docstring says so.
- SPEC_impl.md (around 1870-1875, the embedded helm's tunnel listeners) is the normative text: kernel-selected port,
  bound without `SO_REUSEADDR`, never the remote port number, and why it matters on macOS (a reusable `127.0.0.1` bind
  can shadow another program's wildcard listener, and BSD-derived kernels check only exact-address clashes when picking
  a port for a reusable socket). Linux refuses those binds whatever the flag, which is why no behavioral test catches a
  regression there.
- The plan report that raised this (`git show a0d2ca33^:plans/reports/tunnel-port-and-link-ranges.report.md`, "Open
  questions and possible follow-ups") asked specifically for "a test reading the socket option".

### Acceptance criteria

- A unit test in `tunnels.rs`'s existing `mod tests` calls `bind_listener` and asserts, on the returned listener:
  - the fixture premise, that it is bound on IPv4 loopback;
  - that `SO_REUSEADDR` is off, read back from the socket with `getsockopt`.
- The test fails on Linux if the flag is set to true (or `bind_listener` switches to tokio's `TcpListener::bind`, which
  sets it). Show this once locally by flipping `set_reuseaddr(false)` to true, watching the test fail, and reverting it;
  the mutation is never committed, and the report states the evidence. If you read the option with libc, assert the
  value is 0 rather than comparing against 1: on macOS a set flag can read back as another nonzero value.
- The test follows the file's conventions: `#[farhelm_testtrace::test]`, async (the socket registers with the Tokio
  reactor), and a docstring that says what it pins and why, citing the SPEC_impl.md paragraph and the triage outcome
  `tunnel-shadows-busy-wildcard-port.md` the way its neighbor `tunnel_takes_a_kernel_chosen_port_never_the_remote_one`
  (around 1572-1615) does.
- No product behavior changes. PR type `test:`, so no changelog fragment.
- The PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-10-24, on the planning page:

- **No macOS-only behavioral test.** Since #2635, `bind_listener` takes no target port, so a test cannot steer it onto a
  port another program holds on the wildcard address, and CI has no macOS Rust job (its one macOS job runs the uninstall
  suite). The option read-back is the test the review asked for.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry quoted above, and the decisions above.

**Binding repository constraints:** SPEC_impl.md's tunnel listener paragraph (around 1870-1875); root `AGENTS.md`,
"Tests and validation", "Conventional Commits", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`;
`docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Read the option with `libc::getsockopt` on the listener's raw fd, in an `unsafe` block with a `SAFETY` comment. libc
  is already a helm dependency, and `crates/farhelm-teststate/src/tmux.rs` (around 684-697) has a similar call to
  follow. The alternative is `socket2::SockRef::from(&listener).reuse_address()`, which is safe code but needs a new
  workspace dev-dependency (socket2 is in the lock only transitively). Either is acceptable; log the choice.
- Assert only `SO_REUSEADDR`. The spec names that option alone, and `SO_REUSEPORT` only shares a port with sockets that
  also set it. Leave out the `SO_REUSEPORT` assertion unless the docstring can say plainly that it is a defensive check
  rather than a spec requirement.
- Do not refactor `bind_listener` to expose the `TcpSocket` before `listen`. A test of that intermediate object would
  vanish, rather than fail, if someone switched to `TcpListener::bind`.

## Outline

One PR, `plan/tunnel-bind-reuse-test/01-reuse-option-test`: the test and the TODO.md removal. The `bind_listener`
docstring gains a pointer to the test only if that reads naturally; nothing in it is wrong.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy on `farhelm-helm` with
`--all-targets`, `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares, and
the helm's `tunnels::` unit tests through `python3 scripts/record-test-run.py`. If you add a workspace dependency,
confirm the lock resolves with no other changes.

## Fallbacks

Examples of departures that need the scope reassessment: changing `bind_listener`'s behavior, adding a test seam that
takes a port, or a macOS-only test.

Decisions to log: libc versus socket2, whether `SO_REUSEPORT` is asserted, the mutation evidence, and every review
finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tunnel-bind-reuse-test-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

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
  `plan/tunnel-bind-reuse-test/<nn>-<short-name>`, as the outline names them.
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
