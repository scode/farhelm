# UI: make a refused launch impossible to miss, and warn about a missing folder before launching

Written against main at 71dd7eba on 2026-10-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

It was planned together with `per-host-last-folder` (which also changes the session launcher's folder handling) and
`restart-with-as-launcher` (which runs after this plan has landed and makes Restart with a mode of the session launcher,
so it inherits what this plan builds). `session-view-fixes-oct22`, queued earlier, also edits the session launcher
(Enter on a recent row, and the false "nothing started" message); expect to rebase across it.

## The goal

The TODO.md entry asked for the launcher to tell the user when a session's folder does not exist. It already does: the
supervisor refuses the create (`ensure_cwd_usable`, `crates/farhelm-supervisor/src/service/core.rs`) with
`working directory does not exist: <path>`, the helm passes the text on, and the session launcher
(`crates/farhelm-ui/src/list/create_form.rs`) shows it. But it shows it as one plain red 12px line under the row of
launch buttons (`.create-session-error.launch-composer-refusal` in `crates/farhelm-ui/assets/app.css`), with no box, no
icon, no change to the folder field and no announcement, while the Launch button still reads "launch <agent> · <host> ·
<folder>" exactly as before. The maintainer hit exactly this on 2026-10-22 and did not notice the message.

Deliver:

1. **Prominent refusals.** Every refusal of a launch the session launcher made (New, Clone, Replace with, quick launch)
   appears as a bordered alert with a warning icon beside the launch button, starting with "Not launched." followed by
   the refusal text, and announced to assistive technology. The refusal text itself is unchanged and keeps today's
   peer-text escaping. Errors that are not launch refusals (for example "save as template" refusing wrapper words, which
   today writes into the same error state) are not labelled "Not launched.".
2. **Folder refusals point at the folder.** When the refusal says the folder does not exist, is not a directory, or is
   not usable, the folder field is also marked invalid and a short message under it says so, naming the host (for
   example "This folder does not exist on devbox."). The marking shows only while the host, connection and folder it was
   for are still the current ones, so it disappears as soon as any of them changes.
3. **A check before Launch.** While the launcher is in plain-folder mode (not a new managed checkout) and the target
   host is connected, the launcher asks the host whether the folder exists, 150 ms after the folder or host stops
   changing. When the answer is "does not exist" or "is not a directory", the same message appears under the folder
   field before Launch is pressed. The warning is advisory: Launch still works, and the host still decides at launch
   time. Any other answer (a timeout, a permission problem, a busy host) shows nothing rather than claim the folder is
   missing. A stale answer for an earlier host, connection or path is dropped.

### Acceptance criteria

- The alert, the field marking and the pre-launch warning behave as above, in the light and dark themes and at phone
  width, and match the design the maintainer chose (design C on the planning page): the field marked with a red outline
  and a "⚠ This folder does not exist on <host>." line under it, plus the bordered "⚠ Not launched." alert by the launch
  button.
- Folder refusals are recognized by the supervisor's fixed refusal words (`working directory does not exist: `,
  `working directory is not a directory: `, `working directory is not usable: `), matched with "contains" because helm
  paths can wrap the supervisor's sentence. The same matcher reads the pre-launch check's answer, which carries the same
  words. No protocol change: if the words ever change, the result degrades to the alert without the field marking. The
  marking authorizes nothing, so matching text here does not conflict with the UI's rule that consent and retry
  decisions read structured headers only; say so in a comment.
- The pre-launch check uses the supervisor's existing non-listing directory check, the `membership_only` mode of
  `BrowseDirectory`, which applies the same `~` expansion and existence checks as a create. The helm exposes it through
  a `#[serde(default)]` flag on the existing `/api/browse-directory` body: with the flag it calls the existing
  membership client path (`managed_checkout_for_cwd` or its core), does not list the folder, does not refine the folder
  history, and replies with the existing body shape.
- The launcher's check reuses the existing browse authority (`BrowseAuthority` and `browse_reply_is_current` in
  `create_form.rs`) with its own signal and generation counter, so it never interferes with Browse, inside a
  `use_resource` with a 150 ms sleep like the repository search. It does not start a new check while one is still
  outstanding for the same launcher.
- SPEC_impl.md notes, beside the staleness rule, that the automatic check shares the supervisor's two browse workers, so
  a hung mount can stall Browse on that host until it answers.
- Restart with (`crates/farhelm-ui/src/restart_with.rs`) is not changed by this plan; `restart-with-as-launcher` brings
  it into the launcher afterwards.
- Unit tests cover the folder-refusal recognition. Browser tests pin: the alert for a missing folder, with its text; the
  field marking and its disappearing on folder and host change; the pre-launch warning appearing for a missing folder,
  not appearing for an existing one, and not blocking Launch; a stale check answer being dropped (routing
  `**/api/browse-directory` as `e2e/tests/sidebar.spec.ts` does); a "save as template" error not labelled "Not
  launched.".
- SPEC.md describes the refusal presentation, the folder marking and the advisory pre-launch check (near the launcher's
  working-directory passage and the precondition-failure sentence).
- A `feat:` changelog fragment.
- The last code PR removes the TODO.md entry "If not already fixed, make the launcher tell the user when they try to
  create a session whose directory does not exist." (Near term).

## Decisions already made

Settled with the maintainer on 2026-10-22, on the planning page:

- **The entry is not done, despite the existing message.** The maintainer: "I definitely had this problem today... if
  it's showing and it showed for me it was too easy to miss."
- **Design C:** the alert by the launch button for every refusal, plus the folder field marked when the refusal is about
  the folder. Not either alone.
- **Check before Launch: yes.** The warning is advisory and never blocks Launch.
- **Restart with is left to `restart-with-as-launcher`** (the maintainer chose to make Restart with a launcher mode).
- **Reviewer:** gpt-6.1-sol at high effort (see Review gate). If it cannot be reached when a review is due (CLI missing,
  quota, outage) and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
  "Executing one plan" (step 10), after pushing what is consistent. No substitute reviewer.
- No-workhorse mode.

## Requirement sources

**The user's request:** the TODO.md entry quoted above, the maintainer's report that the message was too easy to miss,
and the decisions above.

**Binding repository constraints:** SPEC.md's session launcher passages (the working-directory field, browsing, and
"precondition failures (nonexistent directory, ...) fail the create with a visible error and no session"); SPEC.md and
SPEC_impl.md on protocol versions (a wire-shape change needs a new protocol version, and hosts on a different version do
not connect); SPEC_impl.md's browse request passages (authenticated, connection-incarnation-guarded; `membership_only`);
root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md";
`.agents/test-authoring.md` for tests.

**Planner proposals** (revised after a planning review): the request flag, the reuse of `BrowseAuthority`, deriving the
field marking from a recorded refusal plus its authority rather than clearing it in each handler, and keeping the
template-save error off the "Not launched." label (derive the label from `launch_attempted`, or route that error to the
existing `template_error`). Change them if the code says otherwise, logged as a DECISION.

A folder that exists but cannot be entered is not a launcher refusal: since #2363 the create is accepted and the failure
shows on the created session. It is out of scope here.

## Outline

A suggested stack; reshape it if the code says otherwise:

1. `feat:` refusals as a prominent alert by the launch button, and the folder field marked for folder refusals (with the
   recognition rule and its unit tests); the template-save error kept off the label; browser tests; SPEC.md; changelog
   fragment.
2. `feat:` the helm's check flag on the browse request, with helm tests and SPEC_impl.md.
3. `feat:` the launcher's advisory pre-launch warning, with browser tests; SPEC.md; TODO.md entry removed.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` (including the UI's web target), the helm and UI unit tests through
`python3 scripts/record-test-run.py`, and narrowly selected browser specs: the launcher's create and refusal paths
(`e2e/tests/terminal-create-idempotency.spec.ts` and its neighbours), folder browsing, and Clone, Replace with and quick
launch refusals.

## Fallbacks

Examples of departures that need the scope reassessment: a protocol change or a new supervisor request for the check,
making the warning block Launch, a general form-validation framework, rewording other refusals, or changing Restart
with.

Decisions to log: the recognition matcher's words, the check's request shape, how the "Not launched." label is kept to
launch refusals, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-launch-refusal-visibility-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/launch-refusal-visibility/<nn>-<short-name>` (for example
  `plan/launch-refusal-visibility/01-prominent-refusals`).
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
