# Skip Claude workspace trust in home, its parents, and root; align Settings and docs

Written against main at 8007a789 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Plan decisions to revisit") plus three related queued review findings under
`review_feedback_queue/`. Verbatim TODO entry:

> **Revisit skipping Claude trust in the home directory and root.** On 2026-11-22 the landing monitor, landing
> `agent-trust-every-folder` (fix #4583), narrowed the maintainer's "every folder" decision for Claude only: with
> Claude's trust switch on, a launch in the home directory or the filesystem root no longer marks it trusted in Claude's
> settings file, prints a note, and leaves Claude to ask. Claude treats a trusted directory as covering every directory
> inside it, so trusting home would trust everything under it, outside Farhelm and after the switch is off, with no
> removal record. The alternative was to apply the edit in every folder as decided, with a caveat in Settings and the
> release note.

Queued review findings addressed and removed by this plan:

- `review_feedback_queue/trailing-claude-trust-guard-allows-home-ancestors-and-fails-open.md`
- `review_feedback_queue/trailing-claude-trust-empty-or-relative-home-edits-cwd.md`
- `review_feedback_queue/claude-trust-settings-omit-folder-exemptions.md`

In product terms: keep skipping Claude's automatic `~/.claude.json` trust edit in the home directory and filesystem
root, extend that guard to also skip **every ancestor directory of `$HOME`** (such as the parent directory containing
user home directories) and fail closed when `$HOME` is unset, empty, relative, or cannot be canonicalized, keep skipping
disposable-environment home directories too, and align Settings → Agents → Claude Code, `SPEC.md`, `SPEC_impl.md`, and
the user docs with this exact behavior.

### Where it stands (from planning research at main 8007a789; line numbers approximate)

- **Supervisor launch guard (`crates/farhelm-supervisor/src/launch.rs:1217-1235, 2021-2029, 2485-2505`):**
  - `claude_trust_covers_too_much(cwd, home)` currently checks only
    `folder.parent().is_none() || home.and_then(|home| std::fs::canonicalize(home).ok()).is_some_and(|home| home == folder)`.
  - It misses proper ancestors of `$HOME`, and fails open when `home` is `None`, empty, relative, or cannot be
    canonicalized (which allows `CLAUDE_CONFIG_DIR` or relative-`HOME` edits to proceed).
  - Fix: require a non-empty, absolute, canonicalizable `home` (failing closed if absent/invalid/unresolvable so the
    guard skips editing rather than writing blindly), and check `folder.parent().is_none() || home.starts_with(&folder)`
    (Rust's path-component `Path::starts_with` matches `/`, every parent of `$HOME`, and `$HOME` itself, while rejecting
    sibling directories that merely share a string prefix like `<homes>/al` vs `<homes>/alice`).
  - Use a 2-variant skip reason (or `Option<&'static str>`) in `launch.rs:1227-1231` so an over-broad directory prints a
    notice naming the home directory, its parents, and `/`, while an unusable `HOME` in an ordinary project folder
    prints an accurate notice explaining that `$HOME` could not be resolved.
- **Claude trust environment resolution (`crates/farhelm-supervisor/src/agent_kind/claude/trust.rs:37-46`):**
  - `Environment::current()` filters empty `CLAUDE_CONFIG_DIR` and `CLAUDE_CODE_CUSTOM_OAUTH_URL`, but does not filter
    empty or relative `HOME` — causing `HOME=""` to resolve `./.claude.json` inside `cwd`.
  - Extract a tiny pure `Environment::from_vars(home, config_dir, custom_oauth)` helper (with `current()` as a 3-line
    wrapper) that filters empty values and requires `home.is_absolute()`, so unit tests verify empty and relative `HOME`
    rejection directly without mutating the test process's environment variables.
- **Settings UI (`crates/gui/farhelm-gui/src/settings/agent_trust.rs:55-67, 200-207`):**
  - `TrustAgent::Claude::mechanism()` (lines 202-207) does not mention the home / parent-of-home / `/` exception. Add a
    concise sentence stating that Farhelm skips your home directory and its parent directories (including `/`) because
    Claude treats a trusted folder as trusting everything inside it, and Claude asks there instead. Refresh and publish
    the `settings-agent-trust` golden scene (and check `settings/search/tests.rs` if indexed text assertions match).
- **Specs & Docs:**
  - `SPEC.md:1782-1784`: update to state the home directory and any ancestor directory above it (including `/`), and
    that an unusable `HOME` fails closed.
  - `SPEC_impl.md:3510-3515`: document the over-broad-directory guard and fail-closed `HOME` rule.
  - `website/src/content/docs/docs/agents/claude.md` and `website/src/content/docs/docs/using/start-a-session.mdx`:
    mention skipping the home directory and its parents (including `/`).

### Acceptance criteria

1. **Supervisor guard and `HOME` validation:**
   - The launch guard skips the trust edit for `/`, `$HOME`, any proper ancestor of `$HOME`, and whenever `HOME` is
     missing, empty, relative, or cannot be canonicalized (printing an accurate stderr notice for each reason). It
     allows sibling directories sharing a string prefix with `$HOME` (e.g. `<homes>/al` when `HOME=<homes>/alice`) and
     subdirectories inside `$HOME`.
   - `trust::Environment::from_vars` / `current()` rejects empty or relative `HOME` values so it can never resolve
     `.claude.json` inside `cwd`.
   - Disposable environments continue to follow the same rule (no new wire plumbing).
   - Unit tests cover `$HOME`, `$HOME`'s parent, `/`, sibling prefix non-match, missing/empty/relative/unresolvable
     `HOME` (including with `CLAUDE_CONFIG_DIR` set), and `Environment::from_vars` rejecting empty/relative `HOME`
     without mutating process environment variables.
2. **Settings UI, goldens, specs, and docs:**
   - Settings → Agents → Claude Code clearly explains that Farhelm skips your home directory and its parent directories
     (including `/`) so Claude does not trust everything under `$HOME`, and Claude asks there instead.
   - `settings-agent-trust` golden screenshots are refreshed and published.
   - `SPEC.md`, `SPEC_impl.md`, `claude.md`, and `start-a-session.mdx` all describe the home/parent/root exclusion.
3. **Bookkeeping cleanup:**
   - Remove the TODO.md entry ("Revisit skipping Claude trust in the home directory and root").
   - Remove all three addressed findings from `review_feedback_queue/` and their entries in
     `review_feedback_queue/INDEX.md` in the same PR.

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- Keep skipping Claude trust in `$HOME` and `/`, and extend the guard to skip **every ancestor/parent of `$HOME`** too
  (explicitly including the queued code review finding about parents of home; _"Neither the home directory nor the root
  nor any parent of home should be recorded as trusted"_).
- Keep skipping disposable-environment home directories as well (no protocol/flag plumbing).
- Update Settings → Agents → Claude Code, `SPEC.md`, `SPEC_impl.md`, and `start-a-session.mdx` / `claude.md` to match.
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: `No release note needed.` (no highlight; folded full list only).

**Binding repository constraints:**

- `review_feedback_queue/AGENTS.md` and root `AGENTS.md`: remove addressed queued items and their `INDEX.md` lines in
  the same commit/PR as the fix.
- `website/AGENTS.md` for docs pages (unattended run: start no preview server, name changed pages).
- `.agents/validation.md` for GUI goldens (`check-gui-goldens.py` + `gui-goldens.py publish`).
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and "Instructions by path".

**Planner proposals:**

- Deliver the entire change as **1 small, cohesive `fix:` PR** (~20 lines of production code across `launch.rs`,
  `trust.rs`, and `agent_trust.rs`, plus unit tests, goldens, specs/docs, 3 queue removals, and 1 TODO removal).

## Outline

1. `plan/claude-trust-home-dir-policy/01-skip-home-ancestors-and-align-docs`: update the launch guard (`launch.rs`) to
   reject `$HOME`, all ancestors of `$HOME` via `home.starts_with(&folder)`, `/`, and absent/invalid `HOME` (failing
   closed with an accurate notice per reason); extract `Environment::from_vars` in `agent_kind/claude/trust.rs` to
   reject empty/relative `HOME` and unit-test it without env mutation; add the home/parent/root explanation in
   `crates/gui/farhelm-gui/src/settings/agent_trust.rs` and refresh/publish `settings-agent-trust` goldens; update
   `SPEC.md`, `SPEC_impl.md`, `website/src/content/docs/docs/agents/claude.md`, and
   `website/src/content/docs/docs/using/start-a-session.mdx`; remove all three addressed `review_feedback_queue/` files
   and their `INDEX.md` lines; add a `kind: fixed` changelog fragment; and remove the TODO.md entry ("Revisit skipping
   Claude trust in the home directory and root").
   `fix: skip Claude workspace trust for home ancestors and invalid HOME`.

## Fallbacks

- If `scripts/gui-goldens.py publish` cannot reach the private goldens storage repository, block with the exact error
  rather than committing a broken manifest.
- If any of the three queued review files was already removed on `main@origin` before execution, record that in the
  working log and skip removing the absent file.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-claude-trust-home-dir-policy-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-claude-trust-home-dir-policy-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run targeted tests in `farhelm-supervisor` (`launch`, `agent_kind::claude::trust`) and `farhelm-gui`
(`settings::agent_trust`, `settings::search`), `check-gui-goldens.py` + `gui-goldens.py publish`, website checks for
edited docs, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `dprint check` on changed
Markdown, and `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it. Apply
`.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live Farhelm
install on this machine (root `AGENTS.md`, "The live install is off-limits").

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
- The last PR of the stack removes the TODO.md entry this plan covers ("Revisit skipping Claude trust in the home
  directory and root") and the addressed `review_feedback_queue/` findings (plus their `INDEX.md` lines), as root
  `AGENTS.md` requires.
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
alternatives considered: in particular those named under Planner proposals and Fallbacks above, and every review finding
you decided not to follow. The maintainer will ask for these later.

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
