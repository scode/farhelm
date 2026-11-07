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

## Decisions

### 2026-11-06: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-11-06 (claim 90feb8).
>
> ### The heartbeat pipe does not fully close a tunnel through the gateway
>
> A force quit or crash on macOS can leave a recipe tunnel's SSH process and sandbox listeners running. The chosen fix
> was a helm-owned stdin pipe, a heartbeat every 30 seconds and a remote 120-second silence limit. You required the
> whole SSH connection to end even with a forwarded connection open, and explicitly required a block if the pipe could
> not do that; no watcher was authorized.
>
> The pipe works against local OpenSSH, but the real Tensorlake gateway failed the held-forward EOF check twice. A fresh
> roundtrip established the forwarded connection before stdin closed. In the diagnostic repeat, independent inspection
> before cleanup confirmed that the remote command and its SSH session process were gone and the sandbox listener
> refused connections. The remote forwarded peer saw EOF/reset. However, after ten seconds the local SSH command's
> result was still pending and the service-side endpoint had seen neither EOF nor reset. The probe did not separately
> inspect the local SSH process table. The gateway appears to retain the forwarded channel after the sandbox-side SSH
> session ends; that explanation is an inference, while the failed teardown is observed.
>
> The failed recorder runs are `dca6ab7d-e116-4341-994a-9a66e1cba923` and `119dfa4b-cdb8-4571-a28a-31739e456a55`. Eight
> focused local tunnel tests passed, including real OpenSSH EOF and silence with a held forward, in
> `f4ce3d72-fd05-48ff-81fd-cca772a05b81`. The macOS unprivileged server prerequisite passed in
> [CI run 42153996017](https://github.com/scode/farhelm/actions/runs/42153996017), using portable OpenSSH with the
> system macOS client. These passes do not resolve the gateway failure. Full gateway silence and lease-kill checks and
> the final macOS regression dispatch were not run because the required EOF guarantee already failed.
>
> [Draft #3072](https://github.com/scode/farhelm/pull/3072/changes) contains only the successful test-server
> prerequisite. The proposed product change remains unpublished and is not ready: its review also found a
> failure-diagnostic issue. Both owned sandboxes and their snapshots were removed, and the test SSH key was
> deregistered. The working log retains the unshipped proposal and evidence.
>
> Should the full teardown requirement remain, with a revised design allowed to investigate a local watcher or another
> gateway-compatible mechanism, or should the requirement be narrowed to ending the sandbox session and listeners while
> allowing the local SSH process and service-side connection to linger? I recommend keeping the full requirement and
> revising the design before implementation resumes; the narrower guarantee leaves local connections behind, and the
> watcher alternative still needs its details settled.

The maintainer's answer:

The maintainer decided on 2026-11-06, in conversation with the landing monitor: "send it back, and make sure that the
agent actually tests it properly to make sure that SSH keep-alive isn't already solving the problem. If that is solving
the problem, proceed with the plan. If that is not solving the problem, block the plan again, and we can discuss."

Why: the tunnel's ssh already runs with `ServerAliveInterval=30` and the default `ServerAliveCountMax` of 3, so it gives
up on a peer that stops answering after about 90 seconds. Both failed held-forward checks gave up waiting after 10
seconds, so they could not show whether keep-alive ends the tunnel's ssh on its own. What the gateway does with a
connection whose sandbox side has ended is not known: if it answers keep-alives itself, the local ssh stays up; if it
does not, keep-alive ends it.

Before any redesign, measure that properly against the real Tensorlake gateway:

- Repeat the held-forward end-of-input check with the tunnel ssh started with exactly the production arguments (the
  shared argument builder, keep-alive options included), and wait at least three minutes after the pipe's write end
  closes, well past keep-alive's timeout.
- Observe the local ssh directly: its own exit status and time (from the owning process's wait, not only a pending
  command result), and whether its process is still in the local process table at the end of the wait. Also record when
  the local service's end of the held connection sees end-of-file or a reset, and keep the existing checks that the
  remote command and session are gone and the sandbox listener refuses connections.
- Show that keep-alive is what ended it: run ssh with `-v` and record its reason for exiting (a keep-alive timeout reads
  "Timeout, server ... not responding"), and run one control with `ServerAliveInterval=0` that stays up for the same
  window. If the control also exits, find out what ended it before drawing a conclusion.
- Retain every run as before and stay within the plan's sandbox budget.

If keep-alive ends the local ssh, the local service's connection and everything in the sandbox, proceed with the plan as
designed. For a forwarded connection held open through the gateway, the local ssh exiting at keep-alive's timeout (about
90 seconds after the sandbox side ends) then satisfies "the tunnel ssh exits"; the sandbox-side criteria keep their
"within a few seconds". Fix the review's open finding about the failure diagnostic, finish the remaining gateway checks
(the silence drop, the lease-drop SIGKILL and idle suspension) and the macOS CI regression, and say in the SPEC.md
Security bullet, SPEC_impl.md and the report that through Tensorlake's gateway the local ssh ends by keep-alive rather
than at once, with the measured times.

If keep-alive does not end it within the wait (the local ssh still running, or the local service's connection still
open), block again with that evidence: the measured times, the `-v` output's reason or its absence, the control run and
the local process table. Do not add a watcher or any other mechanism; the maintainer wants to discuss it with that
evidence in hand.

### 2026-11-07: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-11-06 (claim a3e09b).
>
> ### Keep-alives do not finish the tunnel after its sandbox session ends
>
> A force quit or crash of the desktop app can leave its recipe-tunnel SSH process and sandbox listeners running. The
> chosen fix was a helm-owned stdin pipe, a heartbeat every 30 seconds and a 120-second remote silence limit. The full
> requirement includes ending the local SSH process and its held connection to the local service. Your latest decision
> required a real-gateway check lasting at least three minutes, using production arguments and a control with
> keep-alives disabled, then another block if normal keep-alives did not finish that cleanup.
>
> That check now fails the required local teardown. Both runs established a held reverse connection with a unique
> roundtrip before closing stdin, and observed it for 185 seconds after the write end closed:
>
> | Observation                                                            | Normal keep-alive, 30 seconds / 3 misses | Control, keep-alive disabled |
> | ---------------------------------------------------------------------- | ---------------------------------------- | ---------------------------- |
> | Sandbox peer observed EOF/reset                                        | 94 ms                                    | 85 ms                        |
> | Original remote command and SSH session process gone; listener refused | By 377 ms                                | By 371 ms                    |
> | Local SSH process after 185 seconds                                    | Still present                            | Still present                |
> | Owning process's wait after 185 seconds                                | Unresolved                               | Unresolved                   |
> | Local service's held connection after 185 seconds                      | Still open                               | Still open                   |
> | Verbose SSH exit or keep-alive timeout reason                          | None                                     | None                         |
>
> The treatment used the actual production argument builder and unchanged remote command, including
> `ServerAliveInterval=30` and `ServerAliveCountMax=3`. The control changed only the interval to zero. Direct
> process-table observations before and after the window identified the same local SSH processes; the owning native
> child waits were separate from output draining. Both verbose streams showed the session channel being freed while the
> forwarded channel remained. Why the gateway/client combination leaves that channel open is an inference, not
> established by these diagnostics.
>
> The retained recorder runs are `6c056f69-f776-46c5-8b18-57c7d26a268c` (normal keep-alive) and
> `cd0092d4-81b4-4624-8b34-98c054b394c5` (control). Both measurement commands completed with complete, untruncated
> evidence; their zero command exits mean the observations completed, not that tunnel acceptance passed. This used Linux
> OpenSSH 9.6p1 and closed the pipe in-process, isolating the pipe from Linux's existing kernel parent-death cleanup. An
> independent observer connection deliberately kept the sandbox awake during each window, so these runs make no
> idle-suspension claim or native macOS claim.
>
> One additional small sandbox was used, bringing the plan total to 3 of its authorized 20. It is terminated, its
> snapshot sweep found nothing to remove, and the test SSH key is deregistered. No owned cloud resources remain.
>
> [Draft #3072](https://github.com/scode/farhelm/pull/3072/changes) still contains only the previously successful macOS
> test-server prerequisite. No product fix or alternative lifetime mechanism has been pushed. The remaining silence,
> lease-kill, idle-suspension and final macOS regression checks were not run because this required gateway check failed.
>
> Should the full local teardown requirement remain while a separate local lifetime mechanism, such as a watcher, is
> investigated, or should the requirement be narrowed to ending the sandbox session and listeners while allowing local
> connections to linger? Retaining full teardown requires agreeing on a new local cleanup design, then implementing and
> validating it. A watcher would be a separate local process that notices the helm's exit and stops its tunnel; no such
> design is validated or authorized. Narrowing the requirement accepts a local SSH process and held service connection
> that these measurements did not show closing within 185 seconds. I recommend retaining full teardown and discussing
> the revised design before authorizing its implementation.

The maintainer's answer:

The maintainer decided on 2026-11-07, in conversation with the landing monitor, after the keep-alive measurement in the
block above: recipe tunnels move off their separate ssh connection and onto the helm's existing connection to the
supervisor. In the maintainer's words: "Yes, let's do the supervisor relay. I like this for other reasons as well
because it disconnects it from SSH. [...] you get the one SSH connection, and once you're there, everything else works
the exact same. It limits our exposure to all of these types of issues. We solve it exactly once for the one SSH, and
everything else just floats on top. In the future, if we have other ways of connecting to something that isn't SSH at
all to begin with, all the other stuff on top keeps working, so I vastly prefer this." The design below was worked out
with the maintainer in the same conversation and checked by a fresh-context planning review against the code.

### What this replaces

This decision replaces the plan's goal mechanism (the stdin pipe, the heartbeat, the remote command), its acceptance
criteria, its outline, its fallbacks, its done criterion, and its earlier decisions about the pipe, keep-alive and the
macOS proof. The plan's "How to run" sections still apply (working log, resume protocol, no-workhorse mode, resource
watchdog, PR discipline, scope reassessment, decision logging, unattended fallback), with two changes: the PR-discipline
bullet about dispatching GitHub's macOS CI no longer applies, since GitHub CI is not used for this plan, and the code
review gate is replaced below. The real-sandbox authorization still applies: at most 20 sandboxes over the whole plan, 3
already used, under the same rules.

Why the pipe cannot work: through Tensorlake's ssh gateway, an ssh client holding a remote-forwarded connection never
learns that the sandbox end is gone (`lore/2026-11-07-tensorlake-gateway-forwarded-channels.md`). The supervisor
connection carries no forwarded ssh channel. When the helm dies, the end of its input closes that connection through the
gateway; when the helm's machine goes silent, the sandbox side's 120-second stdin watchdog ends the proxy process, which
closes its socket to the supervisor (`SPEC_impl.md`, the Tensorlake connection section). When a connection ends, the
supervisor drops that connection's relays at once, before its other teardown waits. Tunnels built on that connection
inherit both guarantees on every platform, with no child process left to outlive the helm.

### The design

Settled with the maintainer:

- **The supervisor accepts tunnel connections.** Programs in the sandbox keep connecting to `127.0.0.1:<sandbox port>`
  as today; the supervisor, rather than sshd, listens there. In the maintainer's words: "the spec should allow the
  supervisor to bind to local ports on tensorlake sandboxes for the purpose of forwarding when the user has configured
  such forwards". So SPEC.md's "Supervisors listen on no network port" (it appears twice, in the hosts overview and in
  Security; amend both) gains a narrow exception: IPv4 loopback listeners on a Tensorlake sandbox, on the ports the user
  configured as the host's recipe tunnels, open only while the connection that claimed them lasts. The helm is what
  limits claims to Tensorlake hosts; say so, since the supervisor side does not know its host kind.
- **One transport underneath, everything on top.** The protocol messages and the supervisor's listener handling know
  nothing about Tensorlake or ssh, so they work over any transport the helm uses to reach a supervisor. Only Tensorlake
  recipes configure tunnels for now; no other host kind gets a UI for them.
- **Half-close.** Tunneled connections support half-close, as ssh forwards do today: one side may finish sending while
  still receiving. The existing relays say outright that no half-close exists, so this needs a per-direction
  end-of-input message on the wire and a rule that a relay ends when both directions have ended or on failure. It is
  shared relay code, so the Mac app's localhost tunnels gain it too; keep one relay pump per side rather than a copy.
- **Scheduling stays as it is.** Relay traffic keeps the existing queueing (on the supervisor, bulk traffic drains after
  terminal and control traffic). The near-term TODO entry "Review how the helm-to-supervisor connection multiplexes its
  streams" covers revisiting that; this plan does not change it.
- **At most 64 tunneled connections at once per host**, counting connections waiting for the helm's answer. The
  supervisor closes a connection accepted beyond that at once and logs the reason, and the helm also refuses beyond 64:
  it does not trust the supervisor about how many connections reach the user's own services.
- **No ssh tunnel remains.** Close draft #3072 (the macOS ssh-server CI prerequisite) without merging; the maintainer
  agreed, so this closure is not a rejection of the plan. Nothing depends on ssh forwarding any more.

Planner proposals, from the conversation, the repository research and the planning review (change them if the code says
otherwise, logged as a DECISION):

- **The helm decides where a connection goes.** The supervisor reports only that a claimed port accepted a connection;
  the helm dials the destination from its own snapshot of the recipe, taken when the host connects. Nothing the sandbox
  sends can choose a destination. Only a connection with full helm authority can claim ports; a session-authenticated
  peer (an agent) cannot, as for today's relays.
- **The helm keeps picking every channel id.** On an accepted connection the supervisor sends an event naming the port
  and an accepted-connection id it chose. The helm dials the destination with its own bounded timeout first, then opens
  a relay channel it picked for that accepted connection, or refuses it, and the supervisor closes a refused one. This
  keeps the protocol's rule that the requester picks channels.
- **One claim per connection, all or nothing.** After connecting, the helm claims the recipe's ports once. If any port
  cannot be bound (in use in the sandbox, say), the claim fails as a whole with the reason, as `ExitOnForwardFailure`
  does today, and the helm retries with the existing backoff while the host stays connected. There is no release
  message: the listeners close when the claiming connection ends. A reconnect that finds a port still held by the
  previous connection fails the same way and comes back by retry once that connection's teardown finishes; there is no
  takeover of another connection's listeners.
- **Status stays as users see it today:** Tunnels connecting, up, failed, or retrying with backoff in the host details,
  the amber dot and the attention row. A destination the helm cannot reach fails only that tunneled connection, as with
  ssh today.
- **Recipe rules:** keep the 32-tunnel cap and the port-22 refusal (sshd still owns 22). The destination keeps today's
  rule: any host and port as reached from the helm's machine; strip a bracketed IPv6 literal's brackets before dialing.
  Recipe edits still take effect on the next connection. The hook that starts tunnels on connect has no access to the
  connection today; change its shape as needed.
- **A protocol bump**, as the repository requires for new messages: hosts show Needs update until updated, as with any
  bump.

### Acceptance criteria

- A program in the sandbox reaches the configured destination through `127.0.0.1:<sandbox port>`, both directions,
  including several connections at once, half-close, and transfers larger than one relay window.
- When the helm process dies (SIGKILL), within a few seconds: the sandbox listener refuses connections, a tunneled
  connection held open sees end-of-file or a reset on both ends (the program in the sandbox and the destination
  service), and nothing tunnel-related is left running on the helm's machine.
- When the helm's machine goes silent without the helm dying (SIGSTOP), the sandbox listener goes away within about two
  to three minutes, and nothing tunnel-related is left in the sandbox.
- Orderly paths are unchanged for users: tunnels start when the host connects, stop when it disconnects, is suspended or
  is removed, retry with backoff while it stays connected, and report status and failures as today. After a reconnect,
  tunnels come back by retry.
- A session-authenticated peer cannot claim ports or open tunnel relays, and nothing the supervisor sends can change a
  destination or exceed the 64-connection limit at the helm.
- SPEC.md: the listener exception in both places, and the Security text rewritten so that recipe tunnels are relays over
  Farhelm's own connection rather than an ssh forwarding exception. Keep the existing accepted case that a shared ssh
  connection left by the previous version right after an upgrade can carry the user's own configured forwards. Recipe
  tunnels stop within seconds when the helm dies and within about two minutes when its machine goes silent, as measured.
  Keep "A tunnel alone does not prevent Tensorlake's idle suspension" only if it still holds. SPEC_impl.md's
  recipe-tunnel section is rewritten to the new design and its reasons, and the relay section updated for half-close and
  supervisor-accepted connections. The website pages that say tunnels need direct gateway access and cannot use
  ProxyJump are corrected, following `website/AGENTS.md`.
- A changelog fragment for users (tunnels now end when the desktop app is force-quit or crashes, and work wherever the
  host itself is reachable), and removal of the TODO.md entry "Tunnels outliving a killed desktop helm on macOS".

### Outline

1. `feat:` relays the supervisor accepts: the claim, the accepted-connection event and the helm's answer, per-connection
   listeners and teardown, half-close for all relays, the 64-connection cap on both sides, the protocol bump, SPEC.md's
   listener exception and SPEC_impl.md's relay text, with protocol, supervisor and helm-client tests.
2. `fix:` recipe tunnels over the supervisor connection: the helm's claim on connect, dialing from its recipe snapshot,
   status and retry, removal of the tunnel ssh (its arguments, monitor, Linux parent-death code and process tests), the
   Security text, SPEC_impl.md's recipe-tunnel section, the website pages, the changelog fragment and the TODO.md
   removal.
3. Verification on real sandboxes and on the test Mac, recorded in the report (and in a lore entry if the numbers are
   worth keeping).

Split further if review size demands, without churn.

### Validation

Choose checks per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, and report checks run, reused
and skipped. Expect cargo fmt and clippy, the proto, supervisor and helm tests for what changed through the test
recorder, the browser specs that cover recipe tunnels and Tensorlake host status on Chromium and WebKit,
`python -B scripts/check-test-sleeps.py`, `dprint check`, the changelog check and the docs website build.

- **Process death, locally:** a test that runs a child process using the helm's production client and tunnel code
  against a real supervisor over the local socket, makes the claim, holds a tunneled connection open, kills the child
  (SIGKILL), and checks that the listener closes and both ends of the held connection see end-of-file or a reset. A real
  helm never claims ports over the local socket (it claims only for Tensorlake hosts), which is why the child runs the
  client and tunnel code directly; no ssh is involved.
- **The maintainer's test Mac, not GitHub CI.** The maintainer provided a dedicated Mac for this plan: "a mac box that
  it can go nuts on and use to test. dont use github ci. its free to brew install or whatever things it needs. go nuts.
  its a sandbox for it to playw ith". Its ssh target is the single line in `~/.farhelm-test-mac` on the machine the
  agents run on. Read it there and never write the target into anything public (reports, PRs, commits, Blocked text).
  Install what you need and change what you like on it; nothing on it needs preserving. Run the local process-death test
  there, and run the real-sandbox SIGKILL check below from the Mac as well, since the one macOS-specific link left is
  macOS's own ssh passing a dead helm's end-of-input on to the sandbox. Create sandboxes from the agents' machine as
  usual; for the Mac, generate a throwaway ssh key on it, register it from the agents' machine, and remove it when done.
  Never copy the Tensorlake API key or any other credential to the Mac. Record the macOS version and the recorder ids in
  the report.
- **Real sandboxes**, within the remaining budget: a tunnel works end to end through the real gateway; a SIGKILL of a
  test helm with a tunneled connection held open closes the sandbox listener and both ends within seconds (from the
  agents' machine and from the test Mac); a frozen helm (SIGSTOP) loses its sandbox listener within about two to three
  minutes; and the sandbox still idle-suspends once the host is disconnected (use a short idle timeout for that sandbox
  if the API allows one, otherwise state how long you waited). Use a test helm with its own temporary state, never the
  maintainer's live install.

### Code review gate

Before finishing each PR, two independent fresh-context reviewers review its changes with the plan's existing charter:
one on opus at high effort and one on gpt-6.1-sol at high effort. The maintainer demands exactly these two; no
substitutes and no review swarm. Address both reviewers' findings before moving on. If either cannot be reached after a
reasonable retry, block as the plan already says.

### Fallbacks

Departures that need the scope reassessment: any new process in the sandbox or on the helm's machine; any change to how
tunnels are configured or shown to users; changing the multiplexer's scheduling; letting the supervisor choose a
destination; listener takeover between connections. Block if the supervisor connection's own teardown does not end
tunnels on the real gateway as the acceptance criteria require, if the test Mac cannot be reached, or if verification
would need more than the remaining sandbox budget.

### Done

The plan is complete when a linear stack of draft PRs meets the acceptance criteria above, each PR passed both
reviewers, the test Mac and real-sandbox verification are recorded, #3072 is closed, and the stack removes the TODO.md
entry. Then deliver and close per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12).

A later change of mind by the maintainer is a follow-up, not a reason to stop the plan.
