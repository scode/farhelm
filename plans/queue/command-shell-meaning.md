# Launches: run user-typed commands and wrappers through bash

Written against main at 255fb3a7 on 2026-10-30. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct a spec change.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Expand `~` in the agent command.** A command launch whose program is written as `~/...` fails with `exec_failed` and
> "No such file or directory", because the command is executed without a shell and nothing expands the tilde. Expand a
> leading `~` to the home directory of the user the session runs as, at least in the program, and consider arguments
> too.

On the planning page the maintainer asked whether, instead of expanding `~` by hand, Farhelm could "just actually use a
shell to exec this stuff in both container and non-container cases" so it is expanded when it runs. Shown a tilde-only
design and a full-shell design, with a table of what full shell meaning changes, the maintainer chose full shell
meaning, in their words: "make sure its marked as a breaking chnage in the changelog. no complexity added to try to be
backwards compatible etc."

So the goal is wider than the entry's title: text the user types as a command runs through bash with full shell meaning,
wherever it runs.

How it works today, from a read of main at 255fb3a7 and an independent planning review (line numbers approximate):

- A command launch's command and resume command are split into words with `shell_words::split`
  (`crates/farhelm-proto/src/session_launch.rs`, `start_argv`/`resume_argv` around 430-466; placeholders around 28-56;
  `CommandLaunch::validate` around 141-170). An agent launch's argv is composed by the helm from Farhelm-known parts
  (`crates/farhelm-helm/src/launches.rs`), with the user's wrapper words prepended (around 340). The wrapper is stored
  as a list of words (`LaunchSelection.wrapper_prefix: Option<Vec<String>>`, `crates/farhelm-proto/src/launch.rs` around
  597; also on templates, remembered prefixes and approvals), split by `split_wrapper_prefix` (around 708-715) and
  redisplayed with `shell_words::join` (`crates/farhelm-ui/src/list/create_form/draft.rs` around 1143; other
  `shell_words::join` call sites in `crates/farhelm-ui/src` show wrappers too, in the launcher, templates, approvals and
  session rows).
- The supervisor fills placeholders in `spawn_agent` (`crates/farhelm-supervisor/src/service/core.rs` around
  14840-14900: `{cwd}` and the Codex trust markers via `agent_kind::fill_cwd`, then `{farhelm_args}`, then the initial
  prompt); Restart uses the stored, already-filled resume argv (`relaunch_argv` around 3366-3400,
  `IntegrationSnapshot::filled_resume_argv` in `agent_kind/mod.rs` around 1964). The reserved placeholders are
  `{conversation}`, `{farhelm_args}`, `{cwd}`, `{codex:trusted-cwd}` and `{codex:untrusted-cwd}`.
- The tmux window runs `$SHELL -l -i -c 'exec <farhelm> internal launch <spec>'`
  (`crates/farhelm-supervisor/src/launch.rs` around 657-705). The shim (`exec_launch_spec_with_seam`, around 1066-1200)
  sets PATH and markers after the rc files, scrubs `TMUX`, enters the working directory, writes failure sentinels, and
  then execs `spec.argv` directly (`exec_failed argv0=… errno=…` on failure) or, for a containerized launch, runs
  `podman exec … <argv>` (`crates/farhelm-supervisor/src/container.rs` around 316-326). No shell ever reads the user's
  text.
- Command launches cannot be containerized (SPEC.md around 729); only agent launches with a wrapper put user text into a
  container.

### Acceptance criteria

- **Which text gets shell meaning:** a command launch's command and resume command, and a wrapper, on every host and in
  containerized agent launches. An agent launch with no wrapper is made only of Farhelm's own words and keeps starting
  exactly as today: direct exec, the `exec_failed` status, today's process tree. Legacy sessions (stored invocations
  nothing new creates) also stay literal.
- **The shell:** `bash -c <line>`, non-login and non-interactive, on the host and inside the container. The outer login
  shell still provides the environment, as today. Aliases and functions remain unavailable, as they always were. With a
  simple command, bash execs in place, so the process chain is unchanged. If bash is missing, the shim's existing
  failure sentinel reports it.
- **Command and resume command:**
  - The user's stored text goes to bash byte for byte, except where placeholders are filled in, so newlines, comments,
    `$VAR`, globs, `;`, `&&`, pipes and redirection all mean what they mean in bash.
  - Each placeholder must stand as its own unquoted word. Farhelm inserts its filled value as single-quoted words: one
    word each for `{conversation}`, `{cwd}` and a Codex trust marker, one per argument for `{farhelm_args}`.
  - A placeholder inside quotes, or glued to other text in a way the scan cannot place, is refused when the command is
    saved, with a message saying to write it unquoted. The same check at spawn refuses an older stored row with a clear
    message, rather than silently passing a literal `{cwd}`.
  - The existing once-each and placement rules stay.
- **Wrapper:** each stored word is the word exactly as typed, quotes included; nothing is unquoted or escaped on the way
  in. At launch the wrapper's words are joined with spaces and handed to bash unchanged, followed by Farhelm's composed
  agent argv as single-quoted words.
  - The storage format does not change. Already stored wrappers are read with shell meaning from then on (breaking,
    accepted).
  - The editor's splitter keeps quotes, and its display joins with spaces.
  - Templates, remembered wrappers, approvals and their fingerprints all carry the raw words.
- **The rest of Farhelm keeps reading the literal words:** validation, OMP launch classification, conversation capture,
  the stored resume argv for Restart, and display still read the literal `shell_words` split of the text. Only the bytes
  handed to bash change.
- **A missing program** shows as the command exiting with code 127, with bash's "not found" in the terminal. There is no
  exit-code classification (SPEC_impl.md, around 1876-1879, rejects it), so SPEC.md's Error rule (around 1033, "exec
  failure, command not found") is narrowed to launches that are not run through bash.
- **`~`, `~/…` and `~name`** follow bash's own rules. The planning page's earlier "leave `~name` as written" was for the
  tilde-only design, and is superseded by full shell meaning.
- **Spec and docs** say all of this. In SPEC.md:
  - around 651: the wrapper's "no shell expansion" promise;
  - around 750-771: "as written", placeholders as unquoted words, the Codex marker wording;
  - around 1033: the Error rule;
  - around 2433-2443: the environment contract, now bash parses the line;
  - around 2629-2630: "runs the rest as written";
  - around 3962: bash is needed for these launches whatever the login shell.

  In SPEC_impl.md: around 1876-1894 and 2976-2983. Also the `launch.rs` module docs, the `LaunchSpec.argv` docs, and the
  `ensure_executable_argv` docs. On the website: `agents/agent-wrappers.md` and `agents/custom-commands.md`. SPEC.md's
  rule that file selectors refuse `~` (around 736) stays true.
- **Process attribution:** a compound line keeps bash resident above the program. The pane-position checks for Claude,
  Muse and Codex tolerate that; the Grok and OMP checks (`procs/grok.rs` around 85-110, `procs/omp.rs` around 370-395)
  refuse an unclassified intermediary when a resident wrapper sits below it. Document that consequence. Do not add
  machinery for it.
- **Changelog and TODO:** a changelog fragment with `kind: breaking` explains what changed for someone running Farhelm,
  including examples of commands whose meaning changes (`$`, `*`, `;`, `>`, `&`, `|`, backslashes, quoted placeholders).
  The last code PR removes the TODO.md entry.

## Decisions already made

Settled with the maintainer on 2026-10-30, on the planning page:

- **Full shell meaning, not tilde-only,** for the command, the resume command and the wrapper, on the host and in
  containers.
- **Breaking change, no compatibility machinery:** mark it breaking in the changelog; no migration, no compatibility
  mode, no detection of commands whose meaning changes.
- **bash on every host and in containers,** not `/bin/sh` (on Ubuntu it stays resident and can die on Ctrl+C while the
  agent runs on) and not the login shell (zsh re-reads `.zshenv` and can undo Farhelm's PATH; containers have no
  `$SHELL`).
- **Wrapper words stored exactly as typed, quotes included,** and handed to bash unchanged; no storage format change.
- **Only user-typed text goes through bash;** agent launches without a wrapper are unchanged (from the planning review;
  the maintainer saw it stated).
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's words quoted above, and the decisions above.

**Binding repository constraints:** SPEC.md and SPEC_impl.md passages listed under Acceptance criteria; the per-harness
rules in the module docs of `crates/farhelm-supervisor/src/agent_kind/mod.rs` (root `AGENTS.md`, "Code rules"); the
shim's responsibilities in `crates/farhelm-supervisor/src/launch.rs` (credentials kept out of tmux argv, PATH and
markers after rc, failure sentinels), which stay; root `AGENTS.md`, "Tests and validation", "Conventional Commits",
"Changelog fragments", "TODO.md and FLAKES.md"; `website/AGENTS.md` before editing the docs;
`.agents/test-authoring.md`; `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Build the bash line in the supervisor's `spawn_agent`, where placeholders are filled today, and set
  `spec.argv = ["bash", "-c", line]`; the shim stays unchanged, and its container branch then runs bash inside the
  container. The window command cannot carry the line itself (the shim must keep setting PATH, markers and credentials
  after rc, and handle containers).
- One quote-aware scan of the stored text finds placeholder words standing unquoted; filled values are spliced in as
  single-quoted words and the rest of the text is left byte for byte. Consistency check: every placeholder `shell_words`
  finds must also be found unquoted by the scan.
- Two or three PRs: command and resume command through bash with the placeholder rules; the wrapper as raw words; docs
  and spec may ride with each or come last.

## Outline

1. `plan/command-shell-meaning/01-command-through-bash`: command and resume command run through bash, the placeholder
   scan and splice, save-time and spawn-time refusal of quoted placeholders, tests, the matching spec and docs, and the
   changelog fragment.
2. `plan/command-shell-meaning/02-wrapper-raw-words`: wrapper words kept as typed, the editor's splitter and display,
   launch through bash, tests, the matching spec and docs, the fragment updated, and the TODO.md removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
the touched crates, `python3 releasing/check-changelog.py format`, `python -B scripts/check-test-sleeps.py` with the
interpreter `docs/test-sleep-check.md` prepares, and, through `python3 scripts/record-test-run.py`: the proto and
supervisor unit tests for placeholders, launch specs and resume; the e2e tests for command launches, Restart of a
command launch, wrappers, containerized launches (where the host supports them) and the launch failure status
(`crates/farhelm/tests/e2e/launch_sentinel_error_status.rs`, `session_lifecycle.rs`); and the browser tests that edit
commands or wrappers.

## Fallbacks

Examples of departures that need the scope reassessment: a compatibility mode or migration, classifying exit codes, a
second shell choice, changing the wrapper storage format, routing agent launches without a wrapper through bash, or
changing the shim's responsibilities.

Decisions to log: where the line is built, the scan's exact rules, every placeholder edge case refused or accepted, how
the editor splitter keeps quotes, every spec sentence changed, every test changed and why, and every review finding you
decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-command-shell-meaning-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/command-shell-meaning/<nn>-<short-name>`,
  as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`); this change is breaking, so the type carries `!`. A changelog fragment goes in the same commit as the
  change.
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

## Blocked

Blocked while landing on 2026-10-30 (claim adbcba).

### Landing stopped: two supervisor tests still expect the old command shape

The landing monitor did not merge this plan; its PRs (#2847, #2848) have not merged and main holds nothing of it. The
plan runs agent commands through Bash, so a command written as `~/bin/agent` or with shell syntax means what it would in
a terminal, and hands saved wrapper words to Bash exactly as they were typed.

The whole Rust workspace (`cargo nextest run --workspace --exclude farhelm-desktop`, run
`7ec0376d-1c10-4848-877a-ec3ea8d4c44c`) failed two supervisor unit tests that still expect the old direct launch: "a
confirmed absent create cleans every artifact before replaying failure" expects the launch to run `agent` where it now
runs `bash -c agent`, and "a claude create carries the hook flags into its launch spec" expects `claude` followed by the
hook arguments directly. The executor's test filters did not select them.

A fresh-context review found no unsafe quoting, but things users must be told. Saved wrappers lost their quoting when
they were saved, so after this change `--profile 'work space'` runs as two words, a saved word containing `$`, `*` or
quotes now has shell meaning, and a word starting with `#` turns the rest of the agent command into a comment; this
reaches templates (including ones already approved for agents), remembered setups and plain Restart of existing wrapped
sessions. Containerized sessions with a wrapper now need `bash` in the image (an Alpine image fails); SPEC.md says so,
but the changelog entry and the Containerized sessions docs page do not. On a Mac host, `bash` is usually the system's
old Bash 3.2, which nothing here was tried on. And the plan adds a compiled Bash parser library (`tree-sitter`), which
the release builds (static Linux, macOS) have not tried.

The whole browser suite (run `afafdf3a-2c2c-4d7f-8ef0-28e9e3706ad0`) ran under heavy machine load and failed 11 cases,
some in specs this plan's launches go through (two quick-launch cases, one launcher case and one Replace case on
WebKit).

### What should happen to this plan?

- **Send it back to its executor (recommended).** Answer with: update the two tests to the Bash launch shape; tell users
  in the changelog what changes for saved wrappers and that wrapped container launches need `bash`, and say the latter
  on the Containerized sessions page; try the parser library in a static Linux build; run the whole supervisor package,
  the launch, Restart and container end-to-end tests, and the browser specs that failed here.
- **Land it now and fix afterwards.** Main would carry two failing supervisor tests, and users would meet changed
  wrappers without being told.
