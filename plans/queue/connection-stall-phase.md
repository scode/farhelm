# Show how far a supervisor connection got, and where it stopped

Written against main at 2897c914 on 2026-10-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file says which of their sentences to change.

This plan has no dependency on another plan.

## The goal

When the helm cannot get a working connection to a host's supervisor, the user should be able to tell how far the
connection got and where it stopped, both while an attempt is still running and after it failed. Today the user sees one
sentence that lists every step the attempt might have stuck on and admits it does not know which, and when ssh itself
says why it failed, that reason goes only to the helm's log.

TODO.md entry this plan covers (Near term): "**Say which step of a supervisor connection stalled.** When a session card
reads "Session unavailable — waiting for supervisor", the detail is only "the connection attempt did not complete within
20s (dial, supervisor hello or browser-forwarding setup did not finish)": `deadlined_attempt` in
`crates/farhelm-helm/src/manager.rs` runs dial, supervisor hello, and browser-forwarding placement under one shared
deadline, so expiry cannot say which stalled. Record which phase the attempt is in as it runs and report it on timeout,
and where the transport already knows more — an ssh dial/hello failure with its own text (`annotate_ssh_handshake_eof`
and friends) — surface that instead of the generic sentence, so an operator can tell an ssh tunnel problem (host down,
no route, bad key, no supervisor over there) from a supervisor that accepts connections but never finishes its hello
from a browser-forwarding placement hang. Per-phase deadlines would make it exact, but a phase marker on the shared
deadline may be enough; decide which while planning."

### Acceptance criteria

**The stages.** A connection attempt to a host reports which of these stages it reached, in product terms, in this
order:

1. **Can't reach the host.** ssh could not connect or log in (host down, no route, key refused, a login that hangs).
   After a failure, ssh's own last one or two lines of error output are quoted, escaped and length-limited the way all
   peer text already is (for example "Permission denied (publickey)").
2. **Reached the host, but Farhelm isn't there.** The login worked, but Farhelm could not be started at the configured
   location on the host.
3. **Farhelm is on the host, but no supervisor is running there.** The message includes the command to start one (the
   remedy `annotate_ssh_handshake_eof` already gives).
4. **A supervisor is running but isn't answering.** It accepted the connection and never answered the hello.
5. **The supervisor answered, but the connection never finished setting up.** The last step is the browser-link setup
   (the browser-forwarding acknowledgement). Recording the host's identity in the helm's database sits between stages 4
   and 5; fold it into stage 5 or give it its own wording, and log the choice as a DECISION.

The local host has no ssh, so stages 1 and 2 never apply to it. A failure must never be blamed on a more specific stage
than the evidence supports. Two cases need care:

- **ssh exited with a status other than 255.** Only 255 is ssh's own failure; any other status came from the remote
  command, so the login worked. 127 and 126 (not found, not executable) are stage 2; 75 is stage 3; any other status is
  "reached the host, but Farhelm there failed", quoting the last stderr lines (which then hold Farhelm's own error).
- **A timeout over ssh with no progress line from the remote helper.** A remote running a Farhelm older than this change
  prints no progress line, exactly like a login that is still hanging, so the helm cannot tell those apart on the
  timeout alone. If this helm process has already seen the progress line from this host on an earlier connection, the
  remote is new enough, and the timeout is stage 1. Otherwise the text hedges, for example "ssh had not reached Farhelm
  on
  <host>: still connecting or logging in, or Farhelm there is too old to report its progress" (wording is yours within
  that meaning), plus ssh's last line if it wrote one. The helm keeps no remote version across restarts, and this plan
  does not add one. Live, before the deadline, the same no-line state reads as "connecting to the host…".

**Live and after failure.**

- While an attempt is in flight, the host's connecting state carries the stage the attempt is currently in, and the UI
  shows it in the existing detail line that today reads "attempt N is in flight; the last one failed: …" (and "the first
  connection attempt is in flight"), for example "attempt 2: logging in to the host…". This adds a field to the
  Connecting state published over `/api/hosts`. Keep the publication rate sane: a stage change publishes, but nothing
  publishes on a timer.
- After an attempt fails, by timeout or by an early close, the failure text names the stage it stopped at, and quotes
  ssh's last lines when ssh wrote any (stage 1, and wherever else ssh's own text is the best explanation).
- The card's headline ("Session unavailable — waiting for supervisor") and its explanation stay as they are. The hosts
  panel's detail line carries the same text as the session card's detail.

**What does not change.** The 20-second attempt budget, the retry ladder's timing, and the re-probe cadence stay exactly
as they are. No `ConnectTimeout` (or other ssh option) is added to the supervisor connection's ssh: a user's own ssh
config may set a longer one for a slow link.

**Also:**

- SPEC_impl.md describes the stages and that expiry reports the stage in flight (the paragraph beginning "Two DEADLINES
  bound what a peer can do", which still says "dial and hello together"), that ssh's last stderr lines are retained,
  escaped and bounded, in a failed attempt's text (the paragraph beginning "The ssh child's stderr is piped and
  relayed"), and the remote helper's progress lines wherever the helper's exit-75 contract is described. SPEC.md needs a
  sentence only if it describes the host's connecting state's detail; check, and log what you find.
- The stale UI comment in `crates/farhelm-ui/src/hosts.rs` (around 665-669) that claims ssh's stderr is carried
  unaltered into the state is made true or corrected.
- A changelog fragment per root `AGENTS.md`, "Changelog fragments" (`kind: changed` or `fixed`; log which).
- The last code PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-20/21), from the planning page:**

- The product requirement, in the user's own words when the first version of the question asked about deadlines: "I
  don't care about the implementation details. Obviously, there's an SSH connection. We can tell if that SSH connection
  can't be established at all. SSH won't work. If it does and we can run a shell, we should be able to say to the user
  we are able to reach the host, but something is wrong on the other end, right? Etc. I don't know what the deadline is,
  blah blah. The point is there should be some clear ways we can tell how far we get in the process and give the user
  the actual signal of where something failed."
- The five stages above, reported live and after a failure: "Yes, these five stages".
- Telling an ssh problem from a silent supervisor: the remote Farhelm helper reports its own progress (picked over a
  shell snippet before the remote command, and over exit status and ssh text alone). The option's label named two lines,
  "started" and "connected"; the planner narrowed it to one "connected" line (see the outline), which still yields all
  five stages. The maintainer said the implementation does not matter to them, only the stages.
- ssh's own words: "Yes: its last one or two lines, escaped and length-limited, only after a failure".
- An ssh connect timeout of Farhelm's own: "No".
- Show progress live while connecting: "Yes" (picked against the recommendation, which was to report only after a
  failure).
- Timing is not a product question; the planner chose to keep the shared deadline (see below).
- Review gate and mode are in the run instructions below.

**Binding repository constraints:**

- SPEC_impl.md, the two deadlines paragraph and the 20-second rationale on `CONNECT_ATTEMPT_TIMEOUT`
  (`crates/farhelm-helm/src/manager.rs`, around 159-176): one budget for the whole chain, sized for a cold ssh handshake
  over a bad link plus the remote proxy dialing its supervisor socket. This plan keeps it.
- SPEC_impl.md's ssh stderr paragraph: peer text is control-escaped, bounded, and never inherited; anything quoted into
  a host's state goes through the same normalization (`peer_text` in `manager.rs`, around 660-677).
- The remote helper's stdout is the framed protocol stream. Progress lines go to stderr only, never to stdout.
- Root `AGENTS.md`, "Changelog fragments", "Tests and validation" and "Talking to the user" (the stage wording is for
  users who know Farhelm as a product, not its internals); `.agents/test-authoring.md`; the rule against tests that
  modify their own process environment (inject the ssh program instead of changing `PATH`).

**Planner proposals (checked by a fresh-context planning review):** everything in the outline below, including the
choice of a stage marker on the shared deadline over per-stage deadlines. A planner proposal is not a requirement: a
smaller design that meets the same acceptance criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main (verify them):

- `deadlined_attempt` (`crates/farhelm-helm/src/manager.rs`, around 3461) wraps `attempt` (around 3516) in one
  `tokio::time::timeout`; the timeout message is the TODO's quoted sentence. `attempt` runs the transport connect, the
  hello through `SupervisorClient::start_for_host`, then for an identity-bearing peer `record_first_contact` (around
  3674, a helm.db write), then `accepted_connection` (around 3481), which sends the browser-forwarding setting and
  awaits its acknowledgement. Failures are built by `failure()` (around 4498).
- For an ssh host, the "dial" is effectively instant: `SystemTransport::connect`
  (`crates/farhelm-helm/src/transport.rs`, around 107-158) only spawns the `ssh` child and returns its pipes. TCP
  connect, key exchange, authentication, the remote login shell, and the remote `farhelm internal stdio` helper dialing
  the supervisor socket all happen inside what the helm sees as waiting for the hello. A plain stage marker alone would
  therefore blame almost every ssh stall on the supervisor; that is why the remote helper has to report progress.
- ssh's stderr is drained by `relay_ssh_stderr` (`transport.rs`, around 232) into tracing only; the attempt cannot see
  it. `SshChannel` (around 185) holds the ssh child only for kill-on-drop; its exit status is never read.
- Exit statuses already mean something: `farhelm internal stdio` (`stdio_proxy`, `crates/farhelm/src/main.rs`, around
  2104-2125) exits 75 when the supervisor socket is absent or refuses, and 1 for any other connect error; ssh exits 255
  only for its own failures, and otherwise passes on the remote command's status; a remote shell that cannot find
  `farhelm` exits 127 (126 when it is not executable). Provisioning relies on 75 (`POSITIVE_ABSENCE_EXIT`,
  `crates/farhelm-helm/src/provisioning/backend.rs`, line 24).
- The helper's socket dial is a single `UnixStream::connect` (`farhelm_supervisor::service::connect`). It either
  succeeds or the helper exits at once, so no case leaves the helper stuck before the connect until the deadline; a
  connect that blocks on a full backlog means a supervisor is listening but not accepting, which is stage 4 anyway. That
  is why one progress line, printed after the connect, is enough.
- The helm keeps the remote supervisor's build version only in memory, in the `Connected` state (`build_version`, around
  417-428), and nothing persists it. That is why the older-remote rule above relies on "this helm process has seen the
  progress line from this host before" rather than a version comparison.
- Nearest prior art: provisioning's supervisor probe in `provisioning/backend.rs` (`probe_script` around 1238,
  `ProbeStderr` and `drain_probe_stderr` around 1425-1500, the outcome handling in `probe` from around 1867) already
  recognizes a line-framed stderr marker while keeping a bounded diagnostic prefix, and combines exit status 75, empty
  stdout and the marker into "positively absent". Copy its shape, including its bounded wait for stderr EOF and child
  exit before classifying (otherwise the quoted line is intermittently missing).
- `annotate_ssh_handshake_eof` (`crates/farhelm-helm/src/ssh.rs`, around 478) gives today's two-suspect sentence on
  close-before-hello.
- The Connecting state is `HostState::Connecting { attempt, last_error }` (`manager.rs`, around 394). The UI renders it
  in `state_detail` (`crates/farhelm-ui/src/hosts.rs`, around 640-676); the session card's cause and notice are nearby
  (`session_unavailable_cause`, `stale_session_notice`).
- Tests: the scripted-peer harness in `manager.rs` (`Script`, with `silent_hello` and `silent_browser`), and the
  paused-clock tests `a_peer_that_accepts_and_never_says_hello_still_advances_the_ladder` and
  `browser_forwarding_acknowledgement_is_required_before_connection`, which assert only `contains("did not complete")`
  and pin the ladder timing. ssh annotation unit tests in `ssh.rs`. Real-ssh end-to-end tests in
  `crates/farhelm/tests/e2e/host_connection.rs` (ssh to localhost, skipped where self-ssh is unavailable). Proxy exit
  behavior in `crates/farhelm/tests/stdio_exit.rs`.

Proposed shape (planner proposal), as a linear stack:

1. `fix`: a stage cell owned by `deadlined_attempt` (so it survives the timeout dropping the attempt future), advanced
   by `attempt`, and named in the timeout text. This alone makes the local host exact and separates stages 4 and 5 for
   every host. Extend the two silent-peer tests to assert the stage wording; leave their timing assertions unchanged
   (that is the proof the budget did not move). Changelog fragment.
2. `fix`: a diagnostics handle from the transport next to the reader/writer pair, folded into the existing
   `relay_ssh_stderr` task and `SshChannel`: the ssh child's exit status, its last few stderr lines (bounded), and
   whether the progress line was seen. On close-before-hello over ssh, wait briefly and bounded for stderr EOF and exit,
   then classify: 255 → stage 1 quoting ssh's last lines; 127 or 126 → stage 2 naming the configured remote path; 75 →
   stage 3 with the start-a-supervisor remedy; any other status → "reached the host, but Farhelm there failed" quoting
   the last lines. The timeout text also appends ssh's last line when there is one. Test the classification and line
   framing as pure functions; make the ssh program injectable into the transport (one field defaulting to `ssh`) only if
   a test needs a fake ssh process, never by changing `PATH`. The `HostTransport` trait is dyn and used by test
   transports; widen it, and give the test transport an inert handle.
3. `fix`: `farhelm internal stdio` prints one fixed progress line on stderr once its socket connect succeeds (a literal
   marker, like provisioning's probe marker). The helm's stderr reader recognizes it line-framed and advances the stage
   to 4; the manager remembers, per host and for the life of the helm process, that the host has printed it. At a
   timeout over ssh: line seen → stage 4; no line → stage 1 if this host printed the line on an earlier connection,
   otherwise the hedged older-remote wording. Provisioning's probe parser must keep ignoring the line.
4. `feat`: the live stage: the Connecting state carries the current stage, `/api/hosts` publishes it on change, and the
   UI renders it in the detail line. Edit the fragment from PR 1 rather than adding a second one if it is still
   unreleased. Remove the TODO.md entry.

PRs 2 and 3 can swap. Fold PRs together where a PR would be trivially small; log it.

### Validation

Per root `AGENTS.md`, "Tests and validation", `.agents/validation.md` and `.agents/test-evidence.md`:
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the helm crate's unit tests (the manager,
transport and ssh tests), the `farhelm` crate's stdio exit tests, the real-ssh `host_connection.rs` tests (add two: no
supervisor at the remote state directory gives stage 3; a wrong remote farhelm path gives stage 2), and the UI crate's
tests for the hosts detail line, all through `scripts/record-test-run.py`. New marker-recognition tests copy
provisioning's tests around `backend.rs` 1455-1500 (line framing survives a truncated preceding line). Browser specs
that assert host connecting text, if any (search `e2e/tests/` for "is in flight"), run on both engines. Run the
test-sleep checker for changed Rust and browser tests, and `python3 releasing/check-changelog.py format`.

## Fallbacks

No complexity gate. Examples of departures that need the scope reassessment: per-stage deadlines or any change to the
20-second budget, a new protocol message for progress (the stderr lines exist to avoid one), a new UI surface beyond the
existing detail line, or a shell snippet in the remote command.

Decisions to log: how the identity write folds into the stages; each stage's wording, live and failed; how many ssh
lines are quoted and the length cap; how long the classification waits for stderr EOF and exit; the progress line's
text; the hedged no-line timeout wording; and the fragment's kind. Also log, as a planner decision carried into the run,
that one "connected" line replaces the two lines the maintainer's pick named, and why (above).

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-connection-stall-phase-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user explicitly chose no-workhorse mode. The user forbids sending any unit of your own decomposition of this work to
a weaker model or a lower effort than your own. The review gate and the scope reassessment review are not units of that
decomposition: they keep the model and effort this file demands for them. The user also forbids invoking, loading or
following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already active when you start,
this file is the user's express instruction to stop it.

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
  `plan/connection-stall-phase/<nn>-<short-name>` (for example `plan/connection-stall-phase/01-stage-marker`).
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
