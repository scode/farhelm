# UI: remember the launcher's last folder for each host

Written against main at 71dd7eba on 2026-10-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

It was planned together with `launch-refusal-visibility` (which also changes the session launcher's folder field: a
refusal marking and an advisory "folder does not exist" warning) and `restart-with-as-launcher` (which runs after this
plan has landed). `session-view-fixes-oct22`, queued earlier, also edits the session launcher; expect to rebase across
it.

## The goal

Nothing remembers a last folder today. New starts at the selected session's folder when that session is on the target
host install (`open_new` in `crates/farhelm-ui/src/list/view.rs`), and otherwise at `~` (`LaunchDraft::new` in
`crates/farhelm-ui/src/list/create_form/draft.rs`). Changing the host in the launcher never changes the folder
(`choose_host` in `draft.rs`), so a macOS path is carried over to a Linux host. The maintainer: "I keep having to switch
from like macOS vs Linux paths and other versions of that."

Deliver: for each target host, the launcher defaults to the folder of the user's last successful agent launch there.

### Acceptance criteria

- **What is remembered** is derived, with no new storage, from the launcher's existing recent setups: the helm's
  `launch_history` (`crates/farhelm-helm/src/store.rs`, written only for a user's agent launch from the GUI, which
  covers New, quick launch, Clone and Replace with, and never for agent-created sessions, plain Replace, Restart or
  Restart with), served by `GET /api/launch-history?host=` and already fetched by the launcher per target host
  (`request_launch_history` and `history_target` in `crates/farhelm-ui/src/list/create_form.rs`). A host's remembered
  folder is the `cwd` of its newest row whose `github_repo` is empty, so fresh managed checkouts do not count. Rows are
  keyed by host id and install identity, so a host retargeted to another machine does not carry the memory over.
- **Default folder for New and quick launch:** the selected session's folder when that session is on the target host
  install (today's rule, kept), else the target host's remembered folder, else `~`, which the host expands to its own
  home directory. Quick launch applied on the page with no launcher open reads the target host's recent setups before
  launching (it already does fresh reads for templates).
- **Changing the host in an open launcher** re-seeds the folder from the new host (same precedence) while the folder is
  still a default (`folder_is_explicit` false), and keeps it when the user typed or picked a folder in this draft.
- **Seeding happens once per opening or host choice**, from the first recent-setups reply for that target. Later
  refreshes of the same history (feed notices, another client's launch) do not rewrite the folder of an open draft,
  following the launcher's existing rule that a feed update must not surprise an open composer.
- Clone and Replace with keep prefilling their source session's folder, unchanged. That prefill is a default, so a host
  switch in a Clone draft re-seeds an untouched folder like any other default. Restart with is unaffected. A template
  that sets a folder still wins over the default.
- A remembered folder that no longer exists is used anyway; the launch is refused visibly (and, once
  `launch-refusal-visibility` lands, warned about before Launch), and the next successful launch replaces the memory.
- SPEC.md's working-directory passage no longer says the `~` default "is correct for every target host"; it states the
  precedence above. The quick launch bullet's "uses exactly New's defaults ...: the open session's host and directory,
  else this machine's home" is updated to match. SPEC_impl.md says the default is derived from the newest plain-folder
  row of the recent setups.
- Unit tests for the precedence, the re-seed rule and the seed-once rule in the draft; one or two browser specs for the
  wiring: New opening on a host with a memory (and one without), and a host switch that re-seeds an untouched folder,
  then keeps a typed one.
- A `feat:` changelog fragment.
- The last code PR removes the TODO.md entry "Remember the launcher's most recently selected folder separately for each
  host; for a new host, initially default to that host's home directory." (Near term).

## Decisions already made

Settled with the maintainer on 2026-10-22, on the planning page:

- **What counts:** successful GUI agent launches into a plain folder through New, quick launch, Clone and Replace with;
  not agent-created sessions, Restart with, or fresh managed checkouts.
- **Command-tab launches do not count** (decided after the planning review showed the recent setups record agent
  launches only): reuse the recent setups and add no storage. A command-tab launch leaves the remembered folder as it
  was.
- **Accepted consequence:** recent setups are bounded by the helm's window of the last 100 creates per host install, so
  after 100 later creates there (agent-created and command creates included) the memory falls back to an older
  plain-folder row or to `~`.
- **Precedence:** the selected session's folder (same host) wins, then the host's last folder, then `~`.
- **Host change:** re-seed unless the user typed or picked a folder in this draft.
- **Stale folder:** use it anyway; no existence check before using it.
- **Reviewer:** gpt-6.1-sol at high effort (see Review gate). If it cannot be reached when a review is due (CLI missing,
  quota, outage) and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
  "Executing one plan" (step 10), after pushing what is consistent. No substitute reviewer.
- No-workhorse mode.

## Requirement sources

**The user's request:** the TODO.md entry quoted above, the maintainer's note about macOS and Linux paths, and the
decisions above.

**Binding repository constraints:** SPEC.md's session launcher passages ("Working directory", "Recent setups", "Host",
quick launch) and the rule that only explicit GUI selections recorded after a successful create may shape defaults;
SPEC.md and SPEC_impl.md on preferences being one helm-side value shared by every client, with nothing kept per client;
root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md";
`.agents/test-authoring.md` for tests.

**Planner proposals** (revised after a planning review): derive from `launch_history` in the UI; reuse
`folder_is_explicit` and the existing history target fence rather than new flags. Change them if the code says
otherwise, logged as a DECISION.

## Outline

One `feat:` PR is expected: the launcher's default-folder precedence derived from recent setups, the host-switch re-seed
and the seed-once rule, quick launch's read, unit and browser tests, SPEC.md and SPEC_impl.md, changelog fragment,
TODO.md entry removed. Split it if review would be easier.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` (including the UI's web target), the UI unit tests through
`python3 scripts/record-test-run.py`, and narrowly selected browser specs: opening New with and without a selected
session, the host picker, quick launch, Clone and Replace with.

## Fallbacks

Examples of departures that need the scope reassessment: new storage or a helm change, a per-browser or per-device
memory, counting command-tab launches, a folder existence check, or changing what Clone, Replace with or Restart with
prefill.

Decisions to log: how "chosen in this draft" and "first reply for this target" are detected, and every review finding
you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-per-host-last-folder-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: the review gate keeps the
model and effort this file demands, and the scope reassessment runs at your own model and effort. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewers included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
"Sub-agents this file requires", requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from these rules and run as that file says.

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

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/per-host-last-folder/<nn>-<short-name>`
  (for example `plan/per-host-last-folder/01-host-last-folder`).
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit for `feat` and `fix`.
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
`$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch mechanics). If
gpt-6.1-sol cannot be reached, block as Decisions already made says; never substitute another model.

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
