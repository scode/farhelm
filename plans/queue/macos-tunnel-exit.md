# Tensorlake: recipe tunnels end when the helm dies or goes silent

Written against main at 47b0d59a on 2026-11-05. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

The TODO.md entry (bucket "Near term", subsection "Tensorlake"), verbatim:

> - **Tunnels outliving a killed desktop helm on macOS.** Each recipe tunnel is an `ssh -N -R` child of the helm. An
>   orderly quit stops them, and on Linux the kernel kills them when the helm dies (`PR_SET_PDEATHSIG`), but macOS has
>   no parent-death signal: after a force quit, SIGKILL or crash of the desktop app, the tunnel ssh keeps running until
>   its connection ends on its own, leaving the recipe's listeners open on the sandbox and possibly keeping the sandbox
>   from idle-suspending. Make the tunnels die with the helm on macOS too, for example with a small watcher that notices
>   the helm's exit (kqueue `NOTE_EXIT`) and ends them.

### The mechanism the maintainer chose

Not the watcher the entry suggests: a pipe with a heartbeat, the same liveness rule the supervisor's own connection
uses.

- The tunnel ssh stops being sessionless (`-N`). It runs a small fixed remote command (Farhelm's own string, no user
  input) that reads its stdin and ends when either end-of-input arrives or no input arrives for 120 seconds.
- The helm starts ssh with a pipe as stdin and writes one byte to it every 30 seconds while the tunnel's lease lives.
- When the helm dies for any reason (force quit, SIGKILL, crash), the kernel closes the pipe's write end, ssh reads
  end-of-input and forwards it, and the remote command exits at once. When the helm's machine goes silent (asleep,
  offline) and the gateway drops the connection without the sandbox side noticing, the heartbeats stop and the remote
  command exits within 120 seconds.
- When the remote command exits, it ends its own ssh session on the sandbox side, so the connection and the recipe's
  listeners go away even while a forwarded connection through the tunnel is still open. In OpenSSH's client loop a
  closed session alone does not end the client while any forwarded channel is open, so end-of-input alone is not enough.
- Linux keeps its kernel tether (`PR_SET_PDEATHSIG`) as a second guard; do not remove it in this plan.

The maintainer's words: "the pipe. block the plan if the pipe doesn't work. i don't want to add a watcher without
understanding details of why we need it." After learning that the pipe needs a session channel, and that a silent drop
can leave a channel open inside the sandbox: "just have the other end timeout when not receiving data ... what is the
problem with doing with the heart beat mechanism?" That is the design above.

Why the timeout is not optional: `SPEC_impl.md`, "Tensorlake recipe tunnels and unowned sandboxes", records that the
tunnel connection deliberately has no remote command or session channel, and
`lore/2026-11-03-tensorlake-sandbox-hosts-design.md` measured that an ssh connection with an open channel counts toward
Tensorlake's idle activity, and that after a silent drop by the gateway the in-sandbox side stayed established and kept
the sandbox running and billing. A channel without the timeout would turn laptop sleep into that leak.

Points the executor must get right, from a fresh-context planning review against the code:

- **Remote command:** POSIX sh, since the sandbox login shell may be `dash` (no `read -t`); use coreutils `timeout` in a
  read loop or run it under an explicit `bash -c`, after checking what the sandbox user's shell is. Send its stderr to
  `/dev/null`: it merges into ssh's stderr, and the monitor's diagnostic shows every non-`debug1:` line in the host's
  failure text. Before ending its session by signalling its parent sshd process, confirm the parent is still that
  process (for example compare `ps -o ppid= -p $$` with `$PPID` on the sandbox side), since a stale or reused PID would
  signal an unrelated process of the sandbox user.
- **Options:** remove `-N`; keep `-T` (no pty, so end-of-input passes through as channel EOF) and do not add `-n`.
  `RemoteCommand=none` together with a positional command is fine. `ExitOnForwardFailure=yes` and readiness via the `-v`
  forwarding-confirmation lines are unaffected.
- **The write end:** leave `child.stdin` piped inside the `tokio::process::Child` the tunnel runtime already owns
  (`spawn_owned` takes only stdout and stderr today), so the pipe closes when the child is stopped or the helm dies,
  with no new holder. The heartbeat writer borrows it under the runtime lock the monitor already takes on its tick. No
  other child may inherit the write end; Rust creates pipes close-on-exec.
- **Shared argv:** split `argv` so the tether part (remote command, `-T`, the fixed `-o` set, the `-R` list) is one
  function used by production and the tests, with access options and destination supplied by the caller, so a test copy
  cannot drift. `argv_is_sessionless_unshared_and_recipe_scoped` asserts `-N` today and changes with it.

### Acceptance criteria

- When the process owning a recipe tunnel dies (SIGKILL), the tunnel ends within a few seconds, including while a
  forwarded connection through it is open: the forwarded connection sees end-of-file or a reset, the sandbox-side
  listener refuses connections, and the tunnel ssh exits.
- When the helm's side goes silent without dying, the tunnel ends within about two minutes and nothing Farhelm started
  is left running inside the sandbox.
- Orderly paths are unchanged: tunnels stop when the host disconnects, is suspended or removed, retry with backoff while
  it stays connected, and report status and failures in the host details as today.
- **Local tests** with the real OpenSSH client and server: run `sshd -i` in inetd mode through a `ProxyCommand`
  (absolute path to sshd; host key, `authorized_keys` and config in a temporary directory; `UsePAM no`,
  `StrictModes no`, `PidFile none`) rather than a daemon on a port, so nothing listens, nothing races for a port and
  nothing outlives a failed test; take the remote-forward listen port from a bind to `127.0.0.1:0`. The smallest proof
  drops only the pipe's write end in-process while the lease and a forwarded connection stay open (a dying process
  closes its descriptors exactly that way), plus a case for the 120-second silence (inject a short timeout in the test
  rather than sleeping two minutes). On Linux the parent-death signal kills ssh at the same moment as the pipe closes,
  so only the in-process drop exercises the pipe there. Gate on `sshd` being present with a loud `SKIPPED` on Linux; the
  macOS CI selection must fail, not skip, when sshd is missing (`.agents/validation.md` warns that an early-return test
  is a JUnit pass).
- **macOS CI:** first, one throwaway on-demand dispatch that confirms unprivileged `sshd -i` works on the macOS runner.
  Then add the test to the job (`.github/workflows/ci.yml`, job `uninstall-macos`) in a step of its own. The job does
  not compile `farhelm-helm` tests today, so add a narrow `cargo nextest run -p farhelm-helm --lib -E 'test(<name>)'`
  selection. Dispatch it on your branch (`gh workflow run ci.yml --ref <branch> -f suite=uninstall-macos`) and record
  the run in the report. Update `.agents/validation.md`'s inventory and `.agents/run-all-tests.md`'s lists in the same
  change as `ci.yml`, as root `AGENTS.md` requires.
- **Real gateway:** a local sshd has no gateway, so a green local test does not show the pipe works on Tensorlake.
  Verify on real sandboxes, within the budget under Decisions: end-of-input passes through the gateway when the helm
  dies; the remote command and its session end; nothing is left in the sandbox after a lease-drop SIGKILL of ssh and
  after a silent drop (stop the heartbeat, or freeze the client); and the sandbox still idle-suspends once the host is
  disconnected. Record what you measured, with the sandbox names you used, in the report and in a lore entry if the
  numbers are worth keeping (`lore/AGENTS.md`).
- SPEC.md's Security bullet on recipe tunnels says they end when the helm process dies, and within about two minutes
  when the helm's machine goes silent, as far as you verified; keep "A tunnel alone does not prevent Tensorlake's idle
  suspension" only if it still holds. SPEC_impl.md's "no remote command or session channel" sentence is rewritten to the
  new design and its reason. The docs in `tunnels.rs` (including `spawn_owned`'s "macOS uses lease teardown and the
  helm's orderly shutdown; it has no equivalent parent-death primitive") say how the tether works and why.
- A changelog fragment (`kind: fixed`) about tunnels ending when the desktop app is force-quit or crashes, and removal
  of this TODO.md entry. Do not claim an idle-suspension fix: per the 2026-11-03 measurement a leftover sessionless
  tunnel did not keep the sandbox awake; the harm was the listeners staying open.

## Decisions already made

Settled with the maintainer on 2026-11-05 on planning pages:

- **Mechanism:** the pipe with a 30-second heartbeat and a 120-second silence timeout on the sandbox side, as above.
  **If it cannot be made to work** (on either platform or through the real gateway), push what is consistent and
  **block** with the evidence; do not add a watcher process or any other mechanism without the maintainer.
- **Real sandboxes are authorized** for verifying the gateway behavior: "fine to try a few times. let's say up to 20.
  it's fine. it's cheap. just don't keep them around for longer than necessary for every test." So: create at most 20
  sandboxes over the whole plan, use the smallest size that boots, and terminate each one (and delete any snapshot it
  made) as soon as its check is done. Follow `.agents/tensorlake.md` for creating and naming owned sandboxes, keeping a
  ledger of every sandbox you create and sweeping on interruption; that file's "only on request" rule is satisfied by
  this decision, for this plan only. Register any SSH key the test needs with the signed-in `tl` CLI, and remove it when
  done. Never use the maintainer's live Farhelm install or its helm to create or reach sandboxes (root `AGENTS.md`, "The
  live install is off-limits"); build a test helm with its own temporary state if you need one.
- **Proof on macOS:** a macOS test that runs in the on-demand macOS CI job.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's words and the decisions above.

**Binding repository constraints:** SPEC.md's Security section (Farhelm's ssh connections disable agent, X11 and port
forwarding except the recipe tunnels' explicit forwards; the recipe-tunnel bullet);
`crates/farhelm-helm/src/tensorlake/ tunnels.rs` (one non-multiplexed connection owns all of a host's forwards; lease
drop stops the process group before a suspend or removal hold acknowledges); SPEC_impl.md's recipe-tunnel section and
the supervisor connection's heartbeat and stdin idle limit, which this design mirrors; root `AGENTS.md`, "The live
install is off-limits", "Sharing the machine with other agents", "Tests and validation", "Changelog fragments", "TODO.md
and FLAKES.md"; `.agents/tensorlake.md`; `.agents/test-authoring.md`; `.agents/validation.md`;
`.agents/test-evidence.md`.

**Planner proposals**, refined by a fresh-context planning review against the code (change them if the code says
otherwise, logged as a DECISION): the remote command's shape, the `sshd -i` test harness, the in-process write-end drop
as the main local test, the argv split, and where the tests live. Whether the existing stand-in test in
`tunnel_process_tests.rs` stays: keep it if it still proves something the new tests do not.

## Outline

1. Spike: one throwaway macOS CI dispatch to confirm unprivileged `sshd -i` runs there, and a first real-sandbox probe
   of end-of-input through the gateway. Log both as DECISIONs or block on a negative result.
2. `fix:` tether recipe tunnels to the helm with a heartbeat pipe: the remote command, the argv split, the heartbeat
   writer, the docs in `tunnels.rs`, SPEC.md and SPEC_impl.md, the local tests and their macOS CI step with the
   `.agents/validation.md` and `.agents/run-all-tests.md` updates, the changelog fragment and the TODO.md removal. Split
   the tests or the CI change into a separate PR only if review size demands; never land the behavior without the test
   that proves it.
3. Real-sandbox verification, with the results in the report (and a lore entry if worth keeping).

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`, clippy on the helm crate, the helm
crate's Tensorlake tunnel tests and the new tests through `python3 scripts/record-test-run.py`, the dispatched macOS CI
run, the real-sandbox verification, `dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`,
and `python -B scripts/check-test-sleeps.py` for the new tests.

## Fallbacks

Departures that need the scope reassessment: any watcher or helper process; any change to how tunnels are configured or
reported to the user; changing other Farhelm ssh connections; removing Linux's parent-death signal.

Block, rather than work around, if the pipe or the heartbeat cannot end the real ssh and its sandbox-side session on
either platform or through the gateway, if the macOS CI job cannot run the test, or if verification would need more than
the authorized 20 sandboxes.

Decisions to log: the remote command, the shell it runs under, how the session is ended, the test layout, the measured
gateway behavior, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-macos-tunnel-exit-log.md` in the parent directory of the checkout you run in, derived as that section
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
available as a native sub-agent type (the code reviewer below, for instance), `$scode-harness-shellout` supplies the
launch mechanics.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/macos-tunnel-exit/<nn>-<short-name>` (for
  example `plan/macos-tunnel-exit/01-heartbeat-pipe`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments").
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`). Dispatching
  the on-demand macOS CI workflow on your branch is not marking a PR ready and is required here.

### Code review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, mechanism, acceptance criteria,
decisions and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md`
verbatim in the prompt, as root `AGENTS.md` requires.

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
Wording in this file that turns out inaccurate (a line number, a file name, a class name) is corrected to the truth.

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
passed its code review gate, the macOS CI run that includes the new test passed, the real-gateway verification is
recorded, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest
entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.
