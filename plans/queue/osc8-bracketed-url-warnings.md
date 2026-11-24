# Restore terminal hyperlink mismatch warnings for bracketed and quoted URLs

Written against main at fea806bc on 2026-11-23. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Trivial"). Verbatim:

> **Restore terminal hyperlink mismatch warnings for bracketed and quoted URLs.** Labels opening with a bracket or quote
> (such as `<https://good.test/>`, `"https://good.test/"`, `(https://good.test/)`, or `[https://good.test/]`) pointing
> to a different site stopped triggering the hover mismatch warning after #4737 tightened the scheme rule. Strip one
> matching outer bracket or quote pair before checking whether the label looks like a web address and comparing it with
> the destination, so spoofed bracketed or quoted addresses warn again without false warnings on honest ones.

In product terms, sharpened by the maintainer's planning decisions: when checking whether a terminal hyperlink's visible
label claims to be a web address (and comparing it with the actual target), strip **nested matching outer pairs** from
`<...>`, `"..."`, `'...'`, `(...)`, and `[...]` (with whitespace trimmed around each peeled layer). Spoofed wrapped URLs
— single-wrapped or nested, such as `<https://a.com>` or `("<https://a.com>")` pointing to `https://evil.com` — trigger
the hover mismatch warning again, while honest wrapped links (`<https://a.com>` → `https://a.com`) and prose containing
a URL (`See (https://a.com)`, `bug-1(https://a.com)`, `[docs](https://a.com)`) stay quiet.

### Acceptance criteria

1. **Nested matching outer wrapper stripping:** `crates/gui/farhelm-grid-model/src/link_disclosure.rs` iteratively
   strips matching outer pairs from `<...>`, `"..."`, `'...'`, `(...)`, and `[...]` (trimming JS whitespace around each
   layer) at all three `address_shape` / comparison sites: `mismatch()` (line ~127), `GridModel::link_disclosure()`'s
   fragment check (line ~179), and `GridModel::link_label()`'s multi-row deduplication check (line ~289).
2. **Spoofed wrapped links warn; honest wrapped links and prose stay quiet:**
   - Spoofed `<https://a.com>`, `"https://a.com"`, `'https://a.com'`, `(https://a.com)`, `[https://a.com]`, and nested
     `("<https://a.com>")` pointing to `https://evil.com` all trigger the mismatch warning.
   - Honest matching wrapped links (single or nested, including repeated across adjacent rows under the same OSC-8
     id/uri) stay quiet with no mismatch warning.
   - Prose containing URLs (`See (https://a.com)`, `bug-1(https://a.com)`, `[docs](https://a.com)`) and mismatched
     wrappers (`<https://a.com)`) stay unstripped and do not falsely warn; existing pinned oracle cases including
     `<U+202E>https://example.com/x` continue to pass unchanged.
   - `LinkDisclosure::claimed` and `claimed_text` still display the original trimmed label text when warning.
3. **Tests, changelog fragment, and TODO cleanup:** Unit tests in `link_disclosure.rs` and integration tests in
   `crates/gui/farhelm-grid-model/tests/links.rs` cover all five wrapper types (spoofed and honest), nested pairs,
   multi-row repeated honest wrapped links, mismatched wrappers, and prose cases. Amend
   `releasing/changelog.d/osc8-link-disclosure.md` if still present (since #4737 has not shipped in a stable release),
   update `SPEC.md`/`SPEC_impl.md` if they describe the address recognition rule, and remove the TODO entry.

## Decisions already made

### Maintainer decisions (binding)

- **Wrappers to strip:** All of A–F — nested matching outer pairs from `<...>`, `"..."`, `'...'`, `(...)`, and `[...]`.
- **Review gate:** Single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.`

### Binding repository constraints

- `crates/gui/farhelm-grid-model/src/link_disclosure.rs`: `pinned_disclosure_policy_oracle` must stay intact and pass
  (in particular `<U+202E>https://example.com/x` does not have a closing `>` and must stay unstripped).
- `AGENTS.md`, "Changelog fragments": amending unreleased work edits its existing fragment
  (`releasing/changelog.d/osc8-link-disclosure.md`) rather than adding a second one.

### Planner-chosen implementation shape

- One small pure helper `strip_matching_wrappers(text: &str) -> &str` in `link_disclosure.rs` that trims JS whitespace
  (`is_js_space`), peels one matching outer pair (`<`/`>`, `"`/`"`, `'`/`'`, `(`/`)`, `[`/`]`), and repeats until no
  matching outer pair remains, called by all three sites (`mismatch`, `link_disclosure` fragment path, `link_label`
  multi-row deduplication).

## Outline

### Existing mechanisms reused

- `crates/gui/farhelm-grid-model/src/link_disclosure.rs` (`address_shape`, `mismatch`, `GridModel::link_disclosure`,
  `GridModel::link_label`, `is_js_space`).
- `crates/gui/farhelm-grid-model/tests/links.rs`.

### PR stack

1. **`plan/osc8-bracketed-url-warnings/01-strip-outer-wrappers`** — Strip nested matching outer wrappers in link
   disclosure:
   - Add `strip_matching_wrappers` in `crates/gui/farhelm-grid-model/src/link_disclosure.rs` and route all three
     `address_shape` / `mismatch` call sites (lines ~129, ~179, ~289) through it while preserving the original trimmed
     text for `claimed`/`claimed_text`.
   - Add unit and `tests/links.rs` coverage for all acceptance criteria.
   - Update `SPEC.md`/`SPEC_impl.md` if they state the link-address rule, amend
     `releasing/changelog.d/osc8-link-disclosure.md` if still present, and remove the covered entry from `TODO.md`.

## Fallbacks

- If `releasing/changelog.d/osc8-link-disclosure.md` was already consumed by a release before this plan runs, add a new
  `kind: fixed` changelog fragment instead per `AGENTS.md`, "Changelog fragments".
- Scope-reassessment triggers (examples, not a blacklist): rewriting `ADDRESS` regex broadly to match arbitrary
  punctuation prefixes instead of stripping matching outer pairs, or touching terminal parsing outside
  `link_disclosure.rs`.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-osc8-bracketed-url-warnings-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-osc8-bracketed-url-warnings-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation":

- PR 1: `farhelm-grid-model` unit and integration tests (`links`), `cargo fmt --all -- --check`,
  `cargo clippy -p farhelm-grid-model --all-targets -- -D warnings`, `python3 releasing/check-changelog.py format`,
  `dprint check` on changed Markdown, and `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`.

Apply `.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live
Farhelm install on this machine (root `AGENTS.md`, "The live install is off-limits").

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Restore terminal hyperlink mismatch warnings for
  bracketed and quoted URLs."), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what was
  built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

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
alternatives considered: in particular those named under Planner-chosen implementation shape and Fallbacks above, and
every review finding you decided not to follow. The maintainer will ask for these later.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.
