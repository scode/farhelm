# Settings → Credentials: one storage warning, and a named Tensorlake key

Written against main at c277fa78 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

Two TODO.md entries (bucket "Near term"), verbatim:

> - **Remove the duplicate "Not encrypted on disk" warning in Settings → Credentials.** The page shows the red notice
>   twice, stacked directly above the Tensorlake API key field: first the page-level notice that covers every secret
>   ("Farhelm stores these in plain text…"), then the reused Tensorlake key form's own singular one ("Farhelm stores
>   this key…"). The intent was one warning per surface, so an agent likely wired both into the same panel:
>   `CredentialsSettings` renders `StorageWarning { multiple: true }` and then `TensorlakeKey`, which renders its own
>   warning, and `e2e/tests/credentials-settings.spec.ts` asserts the text appears twice, which locks the duplicate in.
>   Decide whether the key form drops its warning where the page already shows one (keeping it only inside editors) or
>   the two collapse into a single notice, then fix that test.
>
> - **Name the Tensorlake API key in Settings → Credentials.** The key form's visible heading is just "API key", carried
>   over from the Tensorlake page, where the surrounding section supplies the product name; inside Credentials it reads
>   like one of the named secrets and only the help text below says Tensorlake. The shared `TensorlakeKey` component
>   renders `span { "API key" }` for both pages, so label it "Tensorlake API key" at least where `credentials_location`
>   is set, the way its `aria_label` already does.

### Acceptance criteria

- Settings → Credentials shows exactly one "Not encrypted on disk" notice at any time. The overview shows the page-level
  notice once, and the Tensorlake key form shown there renders none of its own. Each credential editor, which replaces
  the overview in place, keeps its own single notice. The page-level wording makes clear that it covers the Tensorlake
  API key as well as the named credentials (for example "Farhelm stores these, including the Tensorlake API key, in
  plain text…").
- Settings → Tensorlake is unchanged: its key form keeps its own singular notice and its "API key" label.
- In Settings → Credentials the key form's visible label reads "Tensorlake API key".
- `e2e/tests/credentials-settings.spec.ts` asserts one notice on the overview (and one in an open editor), and asserts
  the label. The overview stays in the page, hidden, while an editor is open, so count visible notices only. The
  Tensorlake settings spec still sees the singular notice there.
- SPEC.md's Credentials paragraph ("The Credentials overview, every credential editor, and both Tensorlake key forms
  show a prominent danger-tinted **Not encrypted on disk** …") is updated to the new rule.
- Both TODO.md entries are removed in the PR that fixes them.

## Decisions already made

Settled with the maintainer on 2026-11-08 on a planning page:

- **One plan for both entries.**
- **One warning for the whole Credentials section**, in the maintainer's words: "one warning for the entire credential
  sections is what i want". The mechanism: the key form leaves out its warning when `credentials_location` is set.
- **The label changes only in Credentials.** The Tensorlake page keeps "API key" under its own heading.
- **Code reviewer:** a single fresh-context Opus 5.5 agent at high effort with the general charter below.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the two entries and the decisions above. **Binding repository constraints:** SPEC.md's Settings
→ Credentials paragraphs; `website/AGENTS.md` if a docs page names the notice; `.agents/test-authoring.md`;
`.agents/validation.md`; `.agents/test-evidence.md`. **Planner proposals:** the example wording above, and one PR.

## Outline

1. `fix:` one Credentials storage notice and a named Tensorlake key: `crates/farhelm-ui/src/tensorlake_settings.rs`
   (`TensorlakeKey`, `StorageWarning`), `crates/farhelm-ui/src/credentials_settings.rs`, the browser spec, SPEC.md, a
   changelog fragment (`kind: fixed`), and removal of both TODO.md entries. Check the docs site for screenshots or text
   naming the duplicate notice and update text only (screenshots change only through `.agents/docs-media.md`).

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: `cargo fmt`, clippy on `farhelm-ui`, the UI
crate's unit tests if any cover these components, the credentials and Tensorlake settings browser specs on both engines
through `python3 scripts/record-test-run.py`, `dprint check` on changed Markdown,
`python3 releasing/check-changelog.py format`, and `python -B scripts/check-test-sleeps.py` when browser tests change.

## Fallbacks

Departures that need the scope reassessment: changing any other Settings section, or restyling the notice. Decisions to
log: the final wording and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tensorlake-credentials-polish-log.md` in the parent directory of the checkout you run in, derived as that
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
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type,
`$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when a watched filesystem drops
under 10% or under 5 GB free or available memory drops under 10% (whichever comes first), when the number keeps falling,
when the condition recovers, and when the monitor itself fails. In Claude Code a notification is a stdout line of a
monitor started with the Monitor tool; elsewhere, the harness's equivalent. A file update alone is not a notification.

Before relying on the watchdog, verify delivery with a harmless synthetic alert, and verify failure detection by killing
a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a throwaway monitor
without killing it and confirm you are notified within two sample intervals. If any of these notifications is
unavailable or unverified, say so in the log and check the status file at least once a minute. On every path, check
heartbeat freshness before each new build, test run, sub-agent or review launch; a dead monitor, or a heartbeat stale
for two sample intervals, pauses new launches until monitoring is restored.

An alert is an instruction to act: stop launching work, remove jj workspaces, build output and scratch you own that are
no longer needed, wait for or stop the job most likely responsible, and resume only when the watchdog reports headroom.
Record the watchdog's handle, watched paths, status path and delivery mechanism in the log, include its state in every
handoff note, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/tensorlake-credentials-polish/<nn>-<short-name>`.
- Avoid churn; within this run, a PR that needs correcting is restructured rather than corrected on top, including PRs
  an earlier run built.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's
  cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`. PRs stay drafts: never mark one ready and never merge; landing is the monitor's job.

### Code review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 at high effort, launched
natively or through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be
reached when a review is due and is still unreachable after a reasonable retry, block the plan per
`plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. Never substitute another
reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the merits and log the
DECISION.

### Scope reassessment

Before implementing a substantial departure from the outline (examples under Fallbacks; not a blacklist), or when the
same component has needed repeated corrective review rounds, run a fresh-context review at your own model and effort.
Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered, including every review finding you decided not to follow.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Wording in this file that turns
out inaccurate (a line number, a file name, a class name) is corrected to the truth. A material scope expansion, a
weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or
a log entry is not authorization. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you and logged as a DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should
know and possible follow-ups go into the report's "things you should know" and "open questions and possible follow-ups"
sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its code review gate, and the last PR removes both TODO.md entries. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one
plan" (steps 11 and 12). Never edit `plans/` yourself.
