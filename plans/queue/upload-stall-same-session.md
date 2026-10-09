# Detect stalled payload uploads on the upload's own ssh session

Written against main at fd025c7b on 2026-10-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan.

## The goal

When the helm installs or updates Farhelm on a remote host, it uploads payloads (the farhelm binary, tmux, unit files)
over ssh. To catch a stalled upload, it polls the remote file's size every 2 seconds over a second ssh session. On a
host that allows one session per connection and needs an interactive approval for every new login, the upload holds the
only session, every poll fails, the stall timer never starts, and a hung upload can hang indefinitely.

Make the upload report its own progress on its own session, so the stall timer works with no second session.

Acceptance criteria:

- While a payload uploads, provisioning launches no other command to that host (the supervisor's own connection, a
  separate shared connection, is not part of this).
- The stall rule users see is unchanged: the timer starts once the file being sent exists on the host, only growth of
  that file renews it, and an upload that stops growing for 60 seconds fails as stalled. That includes the time after
  the helm has written its last byte, while the bytes still in flight drain to the host.
- The second-session size poll is gone.
- Unit-file installs, which go through the same upload, keep working.
- The code's own documentation (and SPEC_impl.md "Provisioning", if its wording needs it) says where the progress comes
  from.
- The last code PR removes the TODO.md entry "Detect stalled payload uploads without a second ssh session."
- A linear stack of draft PRs as in Outline, each code PR having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, the maintainer's words):** "Detect stalled payload uploads without a second ssh session.
While a payload uploads, provisioning polls the remote file's size over a second ssh session every 2 seconds; the first
successful reading arms the 60-second idle deadline, and only observed growth renews it. On a host whose sshd allows one
session per connection and needs an interactive approval for every new login, the upload itself holds that session, so
every poll is refused and falls back to a fresh login that fails in batch mode (one after another, each bounded by the
30-second command timeout): the deadline never arms, and a hung upload can hang indefinitely. (Without the approval
requirement the fallback login succeeds, at the cost of a full login every poll.) Replace the remote poll with counting
the bytes the helm has pushed into ssh's stdin as the progress signal, and remove the old remote-poll mechanism. Two
things to settle: this reverses the rule, documented in `crates/farhelm-helm/src/provisioning/backend.rs`, that pipe
activity is not progress evidence (local writes run ahead of the remote by the pipe, ssh and TCP buffers, a few MB,
before they block), and nothing local signals progress after the last byte is written while the remote drains those
buffers."

**The user's decision (2026-10-08):**

- D1. Not the TODO's mechanism. The planner laid out two designs: (a) count the bytes the helm writes, as the TODO says,
  which reverses the documented rule and needs a separate time limit for the roughly 2 MB that drains after the last
  byte (a limit that small uploads such as unit files would rely on entirely); (b) have the remote side print the file's
  size every 2 seconds on the upload's own session, keeping today's rule and covering the drain. The maintainer chose
  (b): "Same-session reports". So: no local byte counting, and the documented rule that the remote file's growth is the
  only progress signal stays.
- D2. Review gate: "gpt-6.1-sol high, no swarm."
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:**

- SPEC.md "Provisioning transfers time out only on stalls" (no overall limit; the stall limit starts once the file
  exists; a host that stops answering before then is bounded only by ssh and TCP, accepted 2026-10-01), "Topology" (the
  upload is its own step), and "Supported user environments" (hosts that need approval for every login are supported
  best effort, and only provisioning steps that run one at a time are promised to work there).
- SPEC_impl.md "Provisioning" (the stall paragraph; ssh runs without a connect or keepalive timeout of its own).
- Root `AGENTS.md` (Finishing work, including `scripts/test-provision-centos.sh`; Conventional Commits; Releases and the
  changelog; Sharing the machine with other agents; Agent scratch space; The live install is off-limits),
  `plans/AGENTS.md` (Executing), and `.agents/test-authoring.md` for test changes.

**What the planner verified (at fd025c7b; find code by name, in `crates/farhelm-helm/src/provisioning/backend.rs` unless
noted):**

- `ssh_put` runs `cat > <temporary>` on the host through `ssh_command` (the same command builder every provisioning step
  uses, on the provisioning shared connection), feeds the file into its stdin from a spawned task, and hands the child
  to `capture_transfer_child` with `remote_transfer_size` as the progress probe. Callers: the payload upload
  (`upload_source`) and unit-file writes (`install_source`, via `install_bytes`). After it returns, the caller checks
  the temporary's digest over a separate command; that runs after the upload has ended, one step at a time, and stays.
- `remote_transfer_size` is the second session: `run_shell` with `test -e`/`stat -c %s`, bounded by `COMMAND_TIMEOUT`
  (30 s). Any failure returns `None`.
- `capture_transfer_child` arms the deadline (`TRANSFER_IDLE_TIMEOUT`, 60 s) on the first size reading and renews it on
  growth within the source size, polling every `TRANSFER_PROGRESS_POLL` (2 s). The probe runs inside a `select!` arm, so
  while one is in flight (up to 30 s) child exit and the deadline go unwatched. The child's stdout and stderr are
  drained with a 64 KiB cap (`MAX_CHILD_STREAM_BYTES`).
- The rule's history: #888 (`4cb7fd78`) wrote "pipe activity is not progress evidence" for the old sftp upload, about
  sftp's output; #1143 (`5d9ff144`) switched to `cat >` and kept it.
- No UI shows upload bytes or percent; the numbers never leave `capture_transfer_child`.
- Tests in `crates/farhelm-helm/src/provisioning.rs`: `transfer_capture_uses_remote_bytes_after_slow_setup`,
  `transfer_capture_stall_kills_descendants_despite_output` (real `sh` children, millisecond timeouts passed as
  parameters, no fake clock), and `ScriptedTransferLauncher`, which rewrites the `cat > <temporary>` command for
  `failed_remote_upload_removes_partial_temporary`, `remote_upload_sweeps_orphaned_temporary_before_creating_nonce`,
  `remote_install_rejects_tampering_after_upload`, `remote_unit_write_refuses_a_setup_managed_destination` and
  `ensure_directories_leaves_existing_shared_directories_alone`. The `CommandLauncher` trait is the ssh seam.

**Planner proposals** are the mechanisms in Outline that the decisions do not fix. Challenge them through Scope
reassessment rather than treating them as requirements.

## Outline

### PR 1: report upload progress on the upload's own session

`fix:` with a changelog fragment (kind `fixed`: on a host that allows one ssh session per connection, a stalled upload
while adding or updating the host now fails after a minute instead of possibly hanging forever).

- The remote command becomes a small POSIX `sh` script, still built by `ssh_command` (which runs it as `sh -c` through
  the host's login shell, so it runs under the host's `/bin/sh`: dash on Debian and Ubuntu, bash on CentOS). Run
  `cat > <temporary>` in the FOREGROUND on the session's stdin, and the size reporter in the background: it prints the
  temporary's size every 2 seconds, and the script prints the size once more after `cat` exits, stops the reporter, and
  exits with `cat`'s status. Not the other way round: a background `cat` gets `/dev/null` as stdin in a non-interactive
  `sh`, and under dash even an explicit `<&0` does not undo that (the planning review checked), which would upload
  nothing and fail the digest on every Debian or Ubuntu host. A foreground `cat` also lets the script end as soon as
  `cat` does, rather than up to an interval later, which matters on every upload and after a stall kill (a session slot
  held by a lingering script makes the helm's cleanup command fall back to a fresh login). Redirect the reporter's
  `sleep` away from stdout so it cannot hold the channel open, and send `stat`'s stderr to `/dev/null`, since stderr is
  what a failed upload shows the user. Keep `stat -c %s`, which the old probe already relied on, and nothing beyond
  `sh`, `cat`, `stat`, `sleep` and `kill`. Keep the 2-second interval as the script's named constant (the role
  `TRANSFER_PROGRESS_POLL` has today) rather than a bare `sleep 2`.
- `capture_transfer_child` takes the size readings from the child's stdout instead of calling a probe: a small line
  reader that keeps only the latest size, replacing the capped drain on stdout (nothing else uses the upload's stdout).
  Bound the length of a single line, since output with no newline would otherwise grow without limit (the existing stall
  test prints `noise` with none). A line that does not parse as a size is not progress. Arming and renewal are
  unchanged: the first reading arms, growth within the source size renews. Child exit and the deadline are watched at
  all times.
- Remove `remote_transfer_size`; the helm side no longer needs a poll timer.
- Rewrite the doc comments on `ssh_put` and `capture_transfer_child` to say the remote file's growth is still the only
  progress signal and that the upload's own session reports it. Neither SPEC.md nor SPEC_impl.md mentions a second
  session, so the specs likely need at most a clause in SPEC_impl.md "Provisioning" saying where the size comes from.
- Tests: adapt the existing transfer tests and `ScriptedTransferLauncher` to the new command and the stream; a stall is
  reported when size lines stop growing (including after all input was written); output that is not a size does not
  renew the deadline; an over-long line without a newline is bounded; no other command is launched through the
  `CommandLauncher` while a transfer runs. One test runs the real upload script through the local `sh` (dash on the
  Ubuntu runners and the development host), which is what catches a stdin mistake that bash would hide;
  `ScriptedTransferLauncher` recognizes the upload by calling the backend's script builder rather than copying the
  script's text. The real script runs against a real sshd only in `scripts/test-provision-centos.sh` (bash there).
- Remove the TODO.md entry.

Out of scope: ssh keepalives or connect timeouts; any deadline before the temporary exists (accepted in SPEC.md); a
progress display.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, `python -B scripts/check-test-sleeps.py` (per
`docs/test-sleep-check.md`), the helm crate's tests through `scripts/record-test-run.py` (with the pinned nextest and
tmux setup from `docs/test-run-evidence.md`), and `scripts/test-provision-centos.sh`, the only run of the remote script
against a real sshd. `python3 releasing/check-changelog.py format` for the fragment.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-upload-stall-same-session-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/upload-stall-same-session/<nn>-<short-name>`.
- The stack follows Outline. Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack
  deletes. Within this run, if a PR needs correcting, restructure it rather than stacking a correction on top; that
  applies to all of this plan's own open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entries this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code or tests, use the active galaxy-brain skill to delegate a review of that PR's
changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review swarm: a
fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing
one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria for that PR: its part of Outline, the decisions that apply to it, and
the goal's acceptance criteria. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (counting local bytes, a separate deadline after the
last byte, a progress protocol beyond plain size lines, ssh keepalives, a one-session-per-connection sshd fixture; these
are examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run a
fresh-context review through galaxy-brain with this charter, supplying the user's request and decisions above, this
outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative
was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the remote script's shape, how the line reader bounds its input, and where the
interval constant lives, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code or tests passed the review gate. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
