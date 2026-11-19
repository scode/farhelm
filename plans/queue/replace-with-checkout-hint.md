# Make Replace with on a managed checkout clear

Written against main at d480779c on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Replace with on a managed-checkout session starts on Folder.** Using Replace with on an agent session that runs in a
> managed checkout opens the launcher with Where set to Folder at the session's own folder, not Managed checkout of its
> repository. This is the specified behavior (SPEC.md "Managed checkouts": Replace with and plain Replace start from the
> source's actual directory, while Clone starts on a managed checkout) and the current release does the same, but it
> confuses users. Look into how to make it not confusing. Unrelated to the GPUI migration.

Why it starts on Folder: the folder is the session's own managed checkout. The replacement joins that checkout as a
member before the source is deleted, so it keeps working in the same tree, with its changes. Choosing Managed checkout
instead makes a fresh clone; if the source was the old checkout's last session, that checkout, uncommitted changes
included, then moves to the trash when the source is deleted. The launcher says none of this. The goal is to keep the
behavior and make it obvious.

### Acceptance criteria

1. In Replace with, while Where is Folder, the source session is in a managed checkout, and the Folder field still holds
   the source's own folder (its working directory, which may be a subdirectory of the checkout or reach it through a
   symlink), the launcher shows the session view's checkout mark (the branch glyph and checkout name the session's
   details already draw) beside or leading a line under Folder that names the checkout and says the replacement keeps
   working in it, close to: "This session's managed checkout of owner/repo. The replacement keeps working in it, with
   its changes." Editing or browsing the folder away from that folder hides the mark and the line; going back to it
   shows them again.
2. In Replace with from such a session, choosing Managed checkout under Where shows a warning line under Where. It names
   the source's checkout, not the repository being cloned (the repository field starts empty and the user may pick any).
   For a source on an ordinary host, close to: "Starts from a fresh clone. This session's checkout of owner/repo and its
   changes are not used; it moves to the trash once its last session is deleted." For a source in its own sandbox or
   container, where deleting it discards the checkout rather than moving it to the trash (SPEC.md), close to: "Starts
   from a fresh clone. This session's checkout of owner/repo and its changes are not used; they are discarded once its
   last session is deleted." The wording stays conditional ("once its last session is deleted") because the app cannot
   count a checkout's other members. Choosing Folder again restores criterion 1's line.
3. The Where choices, their labels, the template editor and Clone are unchanged. Plain Replace (no launcher) is
   unchanged. A source that is not in a managed checkout shows neither line.
4. Both lines are labelled for screen readers as the launcher's other help lines are (the warning may use the status
   role the Replace with source warning uses); no new described-by plumbing.
5. SPEC.md "Managed checkouts" (around its lines 1626 to 1631 at planning time) and the Replace with bullet say what the
   launcher shows: PR 1 writes the Folder line's sentence and PR 2 adds the warning's. The user docs pages that explain
   Replace with (`website/src/content/docs/docs/using/start-a-session.mdx` and
   `website/src/content/docs/docs/using/stop-restart-resume.mdx` at planning time) get the same sentence.
   `website/AGENTS.md` governs that edit; per its rules an unattended run starts no preview server and names the changed
   pages in the report.
6. Tests: GPUI tests that Replace with of a checkout session shows the mark and line (including a source whose folder is
   a subdirectory of its checkout), that editing the folder hides them, that choosing Managed checkout shows the warning
   in both variants and Folder restores the line, that a non-checkout source shows neither, and the accessible names. A
   screenshot golden is optional: add one only if pinning the mark visually is worth a reference-image publish
   (`.agents/validation.md`); log the choice.
7. A changelog fragment per root `AGENTS.md`, "Changelog fragments", and the last code PR removes this plan's TODO.md
   entry.

## Decisions already made

Settled with the maintainer on the 2026-11-18 planning page, which showed a mockup of the Replace dialog in both states:

- **The checkout mark plus an explanatory line under Folder.** Rejected: a third Where choice ("This session's
  checkout"), and renaming the choices everywhere.
- **A warning line under Where when Managed checkout is chosen.** The maintainer's condition, verbatim: "assuming this
  is currently supported, i wasn't sure if it even was. if it is, add the warning. if this drags in supporting this and
  it isn't currently, just keep it unsupported but make it obvious when you try to do it that it won't work". The
  planner found it supported today (it makes a fresh clone, then the source is deleted), so the warning is added. If the
  executor finds otherwise, see Fallbacks.
- **Replace with only.** Rejected: also giving Clone switched to Folder the same line.
- **Keep Folder as the default.** It is the only choice that keeps the working tree (planner choice, stated).
- **Release note:** none needed (see the section at the end).
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending**, with no dependency on another plan. The pending plan `drawn-frame-actions` edits the Replace
  with source warning in `launcher/view.rs`; keep the new lines in the launcher's checkout view code
  (`launcher/view/checkout.rs` or its equivalent on main) to stay clear of it.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the Decisions above.

**Binding repository constraints:** SPEC.md "Managed checkouts" (Replace with and plain Replace start from the source's
actual directory; Clone starts on a managed checkout), which this plan keeps; root `AGENTS.md`, "Talking to the user"
for wording, "Code rules", "Changelog fragments", "Conventional Commits", "Tests and validation"; `website/AGENTS.md`
for the docs edit; `.agents/test-authoring.md`.

**Planner proposals** (from reading the code at planning time; follow the code where it differs, logged as a DECISION):

- `LaunchDraft::replace_with` (`crates/gui/farhelm-gui/src/launcher/draft.rs`) seeds only the folder, title and launch;
  `open_replace_with` (`launcher/entry.rs`) documents starting "in its existing directory, even for a managed checkout".
  The source's `SessionInfo.working_copy` already carries the repository and path, so this is client-only: no protocol,
  schema or terminal-host change.
- The desktop app's session list draws no checkout mark yet (`session_list/row.rs` says that form "is not ported yet").
  The only existing mark is the session view's `checkout_mark` (`session_pane/details.rs`: branch glyph, `HostSentence`
  name, hover help); reuse it, with its element id made a parameter, rather than a new mark.
- Read the source session's `working_copy`, `cwd` and whether its host is the session's own sandbox or container from
  the store, live by key, the way the existing Replace with source warning does (`launcher/view.rs`). Do not add a draft
  field, and do not reuse Clone's `clone_source`, which drives Clone's naming. A close call: lift the checkout facts
  `Details::of` computes (`details.rs`: repository text, folder, in-environment) into a small shared function both
  places call, or copy the three lines; log it.
- Draw both lines with the launcher's existing `help_line`; draw the warning in `checkout_rows` beside its existing
  lifetime sentence (`launcher/view/checkout.rs`), as a separate line, since it is a different contract (the old
  checkout, not the new folder). Leading the line with the mark, or placing it beside the field, avoids a new text input
  adornment.
- The folder check (`view/folder_check.rs`) only tests that the directory exists.

## Outline

1. `plan/replace-with-checkout-hint/01-checkout-line`, `fix:`: the mark and line under Folder in Replace with, tests,
   the SPEC.md sentence for the Folder line, and the fragment.
2. `plan/replace-with-checkout-hint/02-new-clone-warning`, `fix:`: the warning under Where in both variants, tests, the
   SPEC.md sentence for the warning, the golden if any, the docs sentences, an edit to PR 1's fragment (the same
   unreleased change), and the TODO.md entry removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
`farhelm-gui`, the launcher's Replace with, checkout and draft tests through `python3 scripts/record-test-run.py`, the
GPUI goldens, the website build or link check `website/AGENTS.md` names for a docs edit, and
`python3 releasing/check-changelog.py format`.

## Fallbacks

- If choosing Managed checkout in Replace with turns out not to work today (it fails, or is refused somewhere), do not
  build support for it: per the maintainer's condition, make it obvious in the launcher that it will not work (a refusal
  line under Where in place of the warning), and say so in the report.
- If the launcher cannot tell that the folder is still the source's own folder from the source's `cwd` and
  `working_copy` the app already has, that is a departure (a new host query, say): run the scope reassessment.

Decisions to log: the final wording of both lines, where they are drawn, how "still the source's checkout" is decided,
every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-replace-with-checkout-hint-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-replace-with-checkout-hint-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation".

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/replace-with-checkout-hint/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's entry for the release, not an instruction to
  you: each PR still leaves the fragment its type calls for, describing what that PR built, and where what got built
  departs from the release note (a renamed control, a part left out, a limitation it does not mention), the fragment's
  context paragraphs say how (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last code PR of the stack removes this plan's TODO.md entry.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
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
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above, each having passed its review gate, and the last code PR removing this plan's
TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close
the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
