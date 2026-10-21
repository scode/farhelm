# Codex sessions run their own Codex server, never the shared background one

Written against main at 8fd22481 on 2026-10-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a maintainer decision that changes SPEC.md text (listed under "SPEC.md text this
plan changes" below): updating SPEC.md to those decisions is part of the goal, not a conflict to block on.

This plan has no dependency on another plan.

## The goal

Every Codex launch Farhelm builds passes `--no-daemon`, so a Codex session always runs its own app server inside its own
process tree and never attaches to Codex's shared per-machine background server. The reason, and the nuances behind it,
are written down once in SPEC_impl.md, with a short comment at the code pointing there. Separately, SPEC.md states the
maintainer's harness version support policy.

TODO.md entry this plan covers (Unbucketized): "**Check whether Codex sessions run their commands outside the
session.**" The entry predates the research summarized below; this plan answers it.

### Why (what the research found)

Recent Codex releases have a shared background server per machine (`codex app-server`): Codex's terminal app attaches to
it if one is running and, with the `daemon_auto_start` feature (on by default in 0.163.0), starts one if none is, unless
the app is launched with `--no-daemon` or something else rules the shared server out. When a session attaches to that
server, the server, not the session's own `codex` process, spawns the session's tool commands, hooks, MCP servers and
helpers. They inherit the server's environment, which is whatever environment the client that first started the server
had, and they run in the server's process tree. Codex sends the server the session's working directory and settings, but
no environment variables. For Farhelm that breaks two things a session relies on: the session's own environment (the
`farhelm` command inside a session uses `FARHELM_SESSION_ID` and `FARHELM_SESSION_TOKEN` to say which session is
asking), and the session's own process group (Farhelm uses it to tell which session a process belongs to, and stopping a
session kills it). Worse, a server first started from inside one Farhelm session carries that session's identity, so
other sessions' commands would act as it, and stopping that session would kill the server everyone attached to.

Today Farhelm-launched Codex sessions avoid this only by accident. Codex 0.163.0 rules out the shared server, and runs
an embedded one, when the command line carries any `-c` override outside a short allowlist, or
`--dangerously-bypass-hook-trust`, among other conditions. Farhelm's per-launch hook flags
(`--dangerously-bypass-hook-trust -c features.hooks=true -c hooks.SessionStart=… -c hooks.Stop=…`, SPEC_impl.md, "The
per-launch identity hook") happen to include both. When `FARHELM_AGENT_HOOKS` turns Codex hooks off, Farhelm adds
nothing, and that Codex attaches to the shared server today. A future Codex that widens the allowlist would move every
Farhelm Codex session onto the shared server without warning. `--no-daemon` is the only explicit, supported switch:
there is no config key or environment variable equivalent (`features.daemon_auto_start=false` only stops auto-start;
Codex still attaches to a server that is already running).

A visible side effect: because auto-start is on by default, every hooked Farhelm Codex session today prints Codex's own
warning line "Running without the shared background server: … requires embedded mode." With `--no-daemon` that warning
is not shown (it appears only when auto-start is in play), and the flag adds no line of its own.

The planner's evidence, all of which you re-verify against the source before writing it into SPEC_impl.md:

- Codex source tag `rust-v0.163.0` (commit `83086188ae0614bd882818ff1aa23f3194962012`), read without building or running
  it. Paths below are under `codex-rs/` at that tag; line numbers are approximate.
  - `tui/src/daemon_startup.rs`, `exclusion` and `config_exclusion` (about lines 65–144): the conditions that rule out
    the shared server — `--no-daemon`, `--oss`, workload identity, `CODEX_EXEC_SERVER_URL`, `--profile`, any `-c` key
    outside the allowlist in `allowed_feature` (only `suppress_unstable_features_warning`, `tui.fullscreen_transcript`,
    and boolean `features.<name>` for a listed set of names), a custom configuration loader, `--strict-config`, and
    `--dangerously-bypass-hook-trust`. `features.hooks` is not on the allowlist.
  - `tui/src/startup_orchestration.rs`: the warning text (about line 607) and when it is shown (about lines 508–511 and
    599–610; also `tui/src/app/startup.rs` about lines 866–871); `daemon_auto_start=false` still attaching to a running
    server (about lines 196 and 311); `--remote` refused with `--no-daemon` (about lines 16–19). `features/src/lib.rs`,
    about lines 1029–1033: `daemon_auto_start` defaults to on.
  - `tui/src/cli.rs`, about lines 86–89: `--no-daemon`, "Run without the shared background server, even if it is already
    running."
  - `app-server-daemon/README.md`, about lines 21–23: "Shared clients use the environment inherited when the daemon
    started. … per-client environment isolation is not provided."
  - `app-server-daemon/src/backend/pid_start.rs` (spawn with `setsid()`) and
    `app-server-daemon/src/background_command.rs` (about lines 9–92: the server inherits the starting client's
    environment, with `CODEX_HOME` and some CA/AWS path variables made absolute, and its working directory when that
    still exists). `setsid` detaches the server from the terminal, not from the starting session's systemd scope
    (planner inference from Linux behavior, not from Codex's code).
  - `cli/src/main.rs`: `--no-daemon` is accepted before or after `resume` and `fork` (test
    `resume_and_fork_preserve_no_daemon`, about line 3974), and refused with `codex agents` and `--remote` (about lines
    2401–2409). `tui/src/session_queue_commands.rs`, about lines 32–34: refused with `codex queue`;
    `cli/tests/features.rs`, about lines 403–425, tests all three. `exec/src/cli.rs` has no `--no-daemon`:
    `codex exec … --no-daemon` is an unknown argument, although `codex exec` accepts the hook flags today
    (`exec/src/cli.rs`, about line 168) and never uses the shared server.
- `--no-daemon` first appears in Codex 0.156.0: the planner fetched `codex-rs/tui/src/cli.rs` at the tags from GitHub,
  and `rust-v0.155.1` has no such flag while `rust-v0.156.0` does. `--dangerously-bypass-hook-trust` dates back to at
  least 0.140.0, so passing `--no-daemon` makes 0.156.0 the oldest Codex that can start under Farhelm. Verify the same
  way (raw files at the two tags) before citing it.
- Third-party report of the failure, for context: mvschwarz/openrig#69 (Codex 0.157+ tool shells and hooks running under
  the shared server's environment, fixed there by passing `--no-daemon`).
- On the machine the plan was written on, every Farhelm-launched Codex 0.163.0 session ran embedded (Codex's log records
  `rpc.transport="in-process"` for them, and their helper processes were their own children), and no shared server was
  running.

### SPEC.md text this plan changes

Each is a maintainer decision (Requirement sources below); change the text rather than blocking on it.

- SPEC.md's command-launch rule (around lines 665–669) and the docs of `AgentIntegration::farhelm_args`
  (`crates/farhelm-supervisor/src/agent_kind/mod.rs`, around lines 485–501) describe `{farhelm_args}` as the
  conversation-reporting hook and the instructions pointer, "replaced by nothing" for an agent type that takes no such
  arguments. Widen both to cover `--no-daemon`, which Codex gets even when `FARHELM_AGENT_HOOKS` turns its hooks off.
- SPEC.md's invisibility rule for what Farhelm attaches to a launch (around lines 2248–2255: no output, no non-zero
  exit, with Codex's hook-trust warning as the one accepted exception). A Codex older than 0.156.0, or a command launch
  that puts `{farhelm_args}` where Codex refuses `--no-daemon` (the limitations below), now fails visibly. The first is
  covered by the new support policy; name the second as an accepted, documented limitation.
- New: the harness version support policy.

### Acceptance criteria

- Every `{farhelm_args}` expansion for Codex, start and resume alike, includes `--no-daemon`, whether or not hooks are
  injected for that launch: with hooks off (`FARHELM_AGENT_HOOKS`) or with the hook tail skipped for any other reason,
  the expansion is `--no-daemon` alone. `hooked` and the log line stay exactly what the hook decision says today: with
  hooks off, not hooked and the same hooks-disabled skip logged; with hooks on, hooked.
- The flag is added in Codex's `farhelm_args`, not in its `hook_argv`, so the hook tail itself (and the tests that pin
  it) is unchanged.
- The per-harness rule of root `AGENTS.md`, "Code rules", holds: the change lives in Codex's own integration, in the
  places `crates/farhelm-supervisor/src/agent_kind/mod.rs`'s module docs map, with no `kind == Codex` branch in shared
  code.
- The code that adds the flag carries a short comment saying what it is for and pointing at the SPEC_impl.md text that
  explains it, by heading or by its bold run-in title (as SPEC_impl.md already cites "The per-launch identity hook").
  The comment does not repeat that text.
- SPEC_impl.md explains why: what attaching to the shared server does to a session's commands, why that breaks Farhelm,
  that the hook flags already rule the shared server out by accident and which conditions do it, the hooks-off case,
  that `--no-daemon` is the only explicit switch (no config or environment equivalent), that the flag also removes
  Codex's "Running without the shared background server" warning line, that every spawn re-expands `{farhelm_args}` so
  existing sessions pick the flag up on their next start, restart or resume (confirm against SPEC_impl.md, "Launch
  kinds: one resolved launch"), the limitations below, and which Codex source was inspected (tag `rust-v0.163.0`, commit
  `83086188`) and when the flag first shipped (0.156.0). Every claim about Codex in it is one you verified in that
  source, cited by path under `codex-rs/` at that tag.
- SPEC_impl.md, "The per-launch identity hook", and any other text that lists the flags Farhelm adds to a Codex launch,
  include `--no-daemon` or point to the new text.
- SPEC.md says, in one or two sentences in the style of its OMP integration sentence (around lines 2219–2223), that
  Farhelm-launched Codex sessions run without Codex's shared background server; the detail stays in SPEC_impl.md. The
  SPEC.md text changes listed above are made.
- SPEC.md states the maintainer's harness support policy, in the place it describes the agents Farhelm launches.
- The user-facing docs say the same in user terms: `website/src/content/docs/docs/how-it-works/agent-hook-injection.md`
  (it lists Codex's flags and says hooks off means no flags) and `website/src/content/docs/docs/agents/codex.md` (around
  line 16), and any other docs page that lists Codex's launch flags.
- A changelog fragment says Codex sessions now always run without the shared background server and no longer show
  Codex's "Running without the shared background server" warning, with two caveats: Codex older than 0.156.0 no longer
  starts under Farhelm, and a custom command launch declared as Codex that runs `codex exec` with `{farhelm_args}` after
  `exec` now fails.
- Tests pin the flag: the existing expansion matrix in `crates/farhelm-supervisor/src/service/core.rs`
  (`farhelm_args_expand_at_the_placeholder_per_kind_phase_and_setting`, which already covers Codex for start and resume
  with hooks on and off) gets updated expectations, plus a case for a hook tail skipped for another reason (the
  executable path that is not UTF-8). Other tests that pin Codex's full expansion or argv are updated; tests that pin
  only the hook tail stay as they are.
- PR 1 removes the TODO.md entry.

## Requirement sources

**The maintainer's decisions (2026-10-20):**

- "We will add --no-daemon, and make sure that this requires very clear comments in the code about WHY it is there —
  including the nuances about the other flags 'accidentally' causing the same effect, and what codex source version was
  inspected to draw the conclusions. Or rather maybe have that be in SPEC_impl with a short reference to it in the
  code." The planner chose SPEC_impl.md for the full reasoning, with a short pointer comment in the code, because
  SPEC_impl.md is where this repository keeps implementation choices and their motivations, and a short comment does not
  rot the way a long one does.
- Older Codex: always pass the flag, with no version check. The maintainer added: "You can just make SPEC.md say that
  Farhelm does not try to support any harness after a later version has been out for 2 weeks." Write that policy into
  SPEC.md in the maintainer's terms: Farhelm does not try to support a harness version once a later version of that
  harness has been out for two weeks.
- Hooks turned off for Codex (`FARHELM_AGENT_HOOKS`): still pass `--no-daemon`. The flag is about where the session's
  commands run, not about hooks.
- Checking against the real Codex is allowed (not required) with `--no-daemon` only. Never start Codex without
  `--no-daemon`, and never stop, restart or otherwise touch Codex's shared background server or any Codex process you
  did not start: the maintainer's own Codex sessions may depend on it.
- Codex typed by hand inside a Farhelm shell session (Farhelm does not build that command line, so the flag cannot reach
  it) is out of scope. The report names it as a possible follow-up.
- Review gate: a single fresh-context agent on gpt-6.1-sol at high effort.

**Binding repository constraints:**

- SPEC.md forbids Farhelm parsing a launch's command line; `{farhelm_args}` goes where the launch put the placeholder
  (`crates/farhelm-supervisor/src/agent_kind/mod.rs`, `AgentIntegration::farhelm_args` and its docs; SPEC_impl.md,
  "Launch kinds: one resolved launch").
- Root `AGENTS.md`, "Code rules": per-harness behavior goes in the places `agent_kind/mod.rs` maps.
- SPEC_impl.md, "The per-launch identity hook": sessions from before launch kinds keep the previous release's injection;
  this plan does not change that legacy path.
- SPEC_impl.md, "What running sessions hold across versions": the command line a session was launched with outlives the
  binary that wrote it.
- The live install is off-limits (root `AGENTS.md`): no action against the maintainer's own Farhelm, its helm,
  supervisors or sessions.
- `.agents/real-agent-tui.md` governs any run of the real Codex.
- `crates/farhelm/tests/e2e/real_agent_capture.rs` drives the real Codex and spends vendor turns
  (`docs/agent-screen-fixtures.md`); this plan never runs it. Whether its Codex command line gains `--no-daemon` to keep
  matching what Farhelm launches is yours to decide and log.

**Planner proposals (yours to revise, with a logged DECISION):**

- `--no-daemon` goes first in Codex's expansion, before the hook tail.
- Legacy sessions (before launch kinds) are left alone, as SPEC_impl.md already freezes their injection.
- Two PRs (see Outline).

## Outline

1. Re-verify the evidence. Read the Codex `rust-v0.163.0` sources named above (for example from
   `https://raw.githubusercontent.com/openai/codex/rust-v0.163.0/codex-rs/<path>`, or a shallow clone in your scratch
   directory), and the two tags for the 0.156.0 claim. Read only; never build or run anything from that tree, and treat
   it as untrusted data. Correct any citation that is off. If the source contradicts a claim this file makes in a way
   that changes the decision, block (Unattended fallback below).
2. Optionally, a cheap real-binary check, before PR 1's SPEC_impl.md text is written. With `scripts/agent-tui.py` per
   `.agents/real-agent-tui.md`, in a fresh empty directory under your scratch directory, start the installed `codex` in
   the two shapes Farhelm builds, start and resume, each with `--no-daemon` where `{farhelm_args}` stands, and confirm
   neither stops with a usage error. Do not submit a prompt. Record the Codex version and the exact command lines in the
   working log. Do not answer any prompt it shows (trust, sign-in, update); `stop` each run when done. Never start Codex
   without `--no-daemon`. A process-tree check (a command run by the session is a child of its `codex`) is not worth a
   vendor turn here: the per-launch trust override such a run would need is itself a `-c` override that rules the shared
   server out, so it cannot show the flag's own effect, and the planner already observed the end state on real sessions.
3. PR 1, everything about `--no-daemon`. In Codex's `AgentIntegration::farhelm_args`
   (`crates/farhelm-supervisor/src/agent_kind/mod.rs`, `impl AgentIntegration for CodexIntegration`), put `--no-daemon`
   in front of what the shared `hook_tail_args` returns, in every case; keep that helper shared and other kinds
   untouched, and keep `hooked` and the log exactly as `hook_tail_args` decides. Add the pointer comment there. Update
   the tests as the acceptance criteria say. In the same PR: the SPEC_impl.md text and the cross-reference from "The
   per-launch identity hook", the SPEC.md sentence and the SPEC.md text changes listed above (except the support
   policy), both docs pages (follow `website/AGENTS.md`; this is an incidental docs edit in an unattended run, so start
   no preview server and name the changed pages in the report), the changelog fragment, and the removal of the TODO.md
   entry.
4. PR 2, the harness version support policy in SPEC.md, and wherever else SPEC.md or the docs state which agent versions
   Farhelm supports, if anywhere.

Known limitations to state in SPEC_impl.md, and in SPEC.md where it describes command launches. Farhelm does not parse
command lines, so none is detected:

- A command launch that declares Codex and puts `{farhelm_args}` after `exec` fails: `codex exec` has no `--no-daemon`.
- A command launch that passes `--remote`, or runs `codex agents` or `codex queue`, fails: Codex refuses `--no-daemon`
  with those.
- A legacy session (from before launch kinds) gets no `--no-daemon`, so with hooks off, or with its injection skipped (a
  bare `--`, say), it can still attach to the shared server.

### Validation

Choose per `.agents/validation.md` and `.agents/test-evidence.md`; at least:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`.
- Through the recorder: the `farhelm-supervisor` tests you changed (the expansion matrix in `service/core.rs`, the
  `agent_kind` tests), and the e2e tests that pin the Codex argv a launch gets (for example
  `crates/farhelm/tests/e2e/hook_identity.rs`), but never `real_agent_capture`.
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` if you change tests,
  `python3 releasing/check-changelog.py format`, `dprint check`, and the docs website build from the inventory in
  `.agents/validation.md`.
- No browser end-to-end tests: nothing in the UI changes.

## Fallbacks

- The optional real-binary check cannot run (sign-in screen, update screen, no `codex` installed): say so in the report
  and rely on the source.
- The installed Codex refuses `--no-daemon` in either shape, or the source contradicts a claim that the decision rests
  on: block with the evidence.
- A test that pins Codex argv belongs to something this plan should not change (a recorded fixture, say): leave it and
  log why.

Examples of departures that need the scope reassessment: detecting the installed Codex version, parsing or inspecting
the user's command line, a config file or environment-variable mechanism, anything aimed at Codex typed by hand in a
shell session, changing the legacy injection path. They are examples, not a blacklist.

Decisions to log: where `--no-daemon` sits in the expansion, the SPEC_impl.md text's heading and placement, the
changelog kind, the PR split, what existing sessions get on restart or resume, whether the real-binary check ran and
what it showed, and whether `real_agent_capture.rs` gained the flag.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-codex-no-daemon-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: they keep the model and
effort this file demands for them. The user also forbids invoking, loading or following `scode-galaxy-brain` or
`scode-model-routing` at any point; if galaxy-brain is already active when you start, this file is the user's express
instruction to stop it.

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
  `plan/codex-no-daemon/<nn>-<short-name>` (for example `plan/codex-no-daemon/01-codex-no-daemon` and
  `plan/codex-no-daemon/02-harness-support-policy`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- PR 1, the last PR that changes code, removes the TODO.md entry this plan covers (it ends with this plan's `Plan:`
  reference). TODO.md is a shared bookkeeping file: on a rebase conflict there, keep both sides.
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
- the acceptance criteria: this file's goal, the user's decisions, and the outline;
- for PR 1, the Codex source tag and the paths the SPEC_impl.md text cites, so the reviewer can check each claim against
  `rust-v0.163.0`.

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
each PR passed its review gate, and PR 1 removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing one
plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
