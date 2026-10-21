# Host updates: stop the false "host changed" warning and say what really changed

Written against main at 2897c914 on 2026-10-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `ui-peer-text-type.md`: the queue holds it back until that plan has landed on main. That plan
converts the host list's text from remote machines (destinations, identities, versions) and the setup and update
progress text in `crates/farhelm-ui/src/provisioning.rs` to a type of its own. This plan rewrites update diagnostics in
that file and, per the user's decision, puts a destination and an identity (both values from another machine) into them,
so it must build on that conversion rather than race it.

## The goal

When the user updates a remote host from the host list, the row follows the update's steps. If the host entry changes
underneath the update, the row stops following it and shows a sticky amber warning. On 2026-10-18 that warning appeared
under an update in which every step completed and the host was green: "the host changed while its update was being
followed, so that run's outcome is unknown; this row claims nothing from it". That was a false alarm, and the wording
explains nothing in terms a user knows. Fix both.

TODO.md entry this plan covers (Near term): "**Rewrite the "host changed while its update was being followed" warning.**
Seen 2026-10-18 on a dev build whose host (running `farhelm 0.0.0-unreleased`, 0 sessions) had also been added by
accident to the maintainer's 0.29.0 helm, which ran setup on it too. The dev build's host row showed an update that
completed every step (create-directories, upload-farhelm, install-farhelm, write-unit skipped because the unit already
had the requested content, daemon-reload, enable-supervisor, enable-linger, restart-supervisor, attach-supervisor), with
the host green, and under it a yellow warning: "the host changed while its update was being followed, so that run's
outcome is unknown; this row claims nothing from it". To the user that reads as nonsense: every step says completed,
nothing says what "the host changed" means or what to do, and "this row claims nothing from it" is internal bookkeeping
language. The warning fires when the row's host, connection kind, destination or install identity changes while an
update is in flight (the update's own reconnect does not count); which one changed here was not investigated. Say in
product terms what changed and what it means for the user (the update may or may not be what is running now; check the
host's version), and consider whether the warning should stay sticky once the host is connected and healthy."

### What the planning session found

The trigger that fired was the install identity, and it was not a real change of host. A helm that adds a host whose
supervisor speaks another protocol registers it with no identity, because the version refusal happens before identities
are exchanged. Updating that host restarts its supervisor as this helm's version, and the update's own final reconnect
records the identity for the first time. The row's continuity check compares identities exactly, so "none" becoming
"some" reads as a change of host, and the watcher detaches the run and shows the warning. This happens every time
someone updates a host they added while it ran an incompatible Farhelm. Confirm this sequence against the code before
building on it.

### Acceptance criteria

1. **No false alarm.** Learning a host's identity for the first time during an update (none recorded before, one
   recorded now) keeps the followed run: the row follows it to its real outcome, shows no warning, and folds on success
   like any update. Any other identity difference (one recorded identity replaced by another, or a recorded identity
   disappearing) still counts as a change of host; failing closed is right there. The same rule applies at both places
   the comparison runs: while the run is being followed, and when the update's submission is accepted.
2. **Wording that names what changed, with old and new values.** The two warnings that report a change of host, the one
   while an update is followed and its twin while the update is being submitted, are rewritten. Each names what kind of
   thing changed and, per the user's decision, its old and new values, in the terms the host row itself shows the user:
   the **ssh destination** (the settings field's own name) and the **identity** (the value the host's detail line shows
   as "identity …"). Each says what it means and what to do. Drafts (final wording is yours within these meanings; log
   it):
   - destination, while followed: "This host's ssh destination changed from ‹old› to ‹new› while it was being updated,
     so Farhelm stopped following that update and can't tell how it ended. Check the farhelm version this host shows
     now, and update it again if it isn't current."
   - identity, while followed: "This host's identity changed from ‹old› to ‹new› while it was being updated (a different
     Farhelm install now answers at its ssh destination), so Farhelm can't tell how that update ended or which install
     it reached. Check the farhelm version this host shows now, and update it again if it isn't current."
   - the submitting twin: the same two sentences, saying the change happened while the update was being sent and so
     Farhelm can't tell whether the update started.
   - a change of entry or connection kind (not expected in practice): a fallback sentence with no values, "This host's
     entry changed while it was being updated, so Farhelm can't tell how that update ended. Check the farhelm version
     this host shows now, and update it again if it isn't current."

   The user's instruction for all of it: "Make sure the wording contains user-visible terminology and not any kind of
   internal jargon, unless it is completely impossible to avoid." So no "row", "run", "claims", "binding", "target" or
   other internal word. Values from the host are shown escaped, the way the host row shows them (today the warning is
   already rendered as one escaped peer part, or through whatever text type `ui-peer-text-type.md` left there); an
   identity that is absent on one side is shown with the row's own phrase, "no identity reported". Both changing within
   one update is near impossible (a destination edit keeps the recorded identity; only adopt changes it), so it needs no
   wording of its own: join the two first clauses, or use the destination sentence alone, and log which. Match the
   capitalization and final punctuation of the other messages in the same place (the untouched update diagnostics are
   lowercase without a final period); log the choice.
3. **The warning clears once the question is moot.** A change-of-host warning clears when the host is connected and runs
   the same farhelm version as this helm. It stays while the host is not connected or runs an older or newer version,
   and every existing way of clearing it (a later successful update, removing the row, reloading the page) still works.
   Decide how "the same version as this helm" is read from what the UI already has, and log it. A hint: a connected row
   carries only the host's reported build; the UI's own build is `skew::CLIENT_BUILD` (and the helm's reported build
   under skew, as `app_bar.rs` uses). Under this rule a genuine retarget to a host that already runs this helm's version
   clears on the next host-list read, so the user may barely see the warning; that is what the maintainer chose. Say so
   in the report.
4. The other update diagnostics in the same file (lost reply, the helm accepting an update for another host, no run
   identity, a run that ended without reporting) are out of scope and keep their text.
5. A changelog fragment, `kind: fixed`.
6. The last code PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-20), from the planning page:**

- Stickiness: "Clear it once the host is connected and runs this helm's version", chosen over keeping today's sticky
  behavior with new words and over clearing it on any next click or host-list change.
- Scope: "This warning plus its twin "the host changed while the update was submitting"", chosen over rewriting every
  update diagnostic and over this one sentence only.
- Values: "Yes, include both" (the old and new values), chosen over naming only the kind of change, with the comment
  "Make sure the wording contains user-visible terminology and not any kind of internal jargon, unless it is completely
  impossible to avoid."
- The false-alarm fix was presented on the planning page as part of this plan's shape and not questioned.
- Review gate and mode are in the run instructions below.

**Binding repository constraints:**

- SPEC.md's host identity passages (identity mismatch and adoption, around SPEC.md:185-222) and update behavior; the
  UI's rule that text from another machine is shown escaped (`crates/farhelm-ui/src/peer.rs` module docs, and whatever
  `ui-peer-text-type.md` put on main).
- The docs of `UpdateDiagnostic` and the binding watcher in `crates/farhelm-ui/src/provisioning.rs`, which record why
  the diagnostics' lifetimes are shaped as they are; keep the new clearing rule a separate, narrow condition.
- Root `AGENTS.md`, "Changelog fragments" and "Tests and validation"; `.agents/test-authoring.md`.

**Planner proposals (checked by a fresh-context planning review):** the outline below. A planner proposal is not a
requirement: a smaller design that meets the same acceptance criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main at 2897c914 (verify them; `ui-peer-text-type.md` will have moved some
of this code):

- `HostBinding` (`crates/farhelm-ui/src/provisioning.rs` around 307-329) is built from the host list row; its `identity`
  is the helm registry's recorded identity. `same_update_target` (around 331-347) compares id, kind, destination and
  identity with `==`, so `None` to `Some` counts as a change.
- The binding watcher (around 2636-2685) detaches the followed run and sets the sticky warning; the acceptance-time
  check and the "while the update was submitting" warning are around 1559-1594.
- Why `None` becomes `Some`: the helm registers a version-skewed host without an identity
  (`crates/farhelm-helm/src/provisioning/service.rs` around 574-601 and 1722-1745), and the first hello after the
  update's restart records it (`crates/farhelm-helm/src/manager.rs` around 3672-3684; store semantics
  `crates/farhelm-helm/src/store.rs` around 6540-6573).
- Stickiness: `UpdateDiagnostic.sticky` (around 548-565) is cleared only by a correlated newer update success, row
  removal or a reload; `update_auto_disclosed` (around 587-601) keeps the row's details open while any update warning
  stands. The warning renders as a `.host-warning.provisioning-warning` line (around 2997-3000 and 3286-3291).
- The host detail line (`crates/farhelm-ui/src/hosts.rs` around 677-697) shows "farhelm ‹version›; identity ‹id›" or ";
  no identity reported"; the settings field is "ssh destination" (around 4070 and 4448).
- Tests: unit `run_continuity_survives_reconnects_but_not_retargets` (around 3743-3765) and
  `competing_run_explanation_yields_to_a_sticky_warning` (around 4320-4360); browser `e2e/tests/provisioning.spec.ts`
  "incarnation churn keeps a followed run while a target change detaches it" (around 3700-3800, asserting the old text)
  and the "while the update was submitting" assertions (around 4030-4090 and 4236-4255). No test updates a host
  registered without an identity.

Proposed shape (planner proposal), as a short stack:

1. `fix`: the first-learned identity keeps continuity, with a docstring saying why; unit test; a browser test that
   rewrites the row's identity from none to a value mid-run and sees the run followed to completion with no warning;
   changelog fragment.
2. `fix`: format the sentence where the warning is raised, from the two bindings both places already hold (the watcher
   has the followed run's binding and the current one; the acceptance check has the plan's binding and the current one),
   into the diagnostic's existing text, so no new data model carries the values. Add one narrow marker on
   `UpdateDiagnostic` (a bool or a two-value kind) that says "this is a change-of-host warning", which is all the
   clearing rule keys on. With it: the new wording for the followed and submitting cases, the clearing rule, updated and
   new tests (destination change shows the destination wording with both values; identity change shows the identity
   wording; the warning clears once the row reports connected at this helm's version and stays while it reports another
   version). Edit PR 1's fragment rather than adding a second.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the UI crate's unit tests, and `e2e/tests/provisioning.spec.ts` through the
recorder on both engines; `python -B scripts/check-test-sleeps.py` for changed browser tests;
`python3 releasing/check-changelog.py format`. Optionally, a helm-side test pinning the premise (an update of a
version-skewed host ends with the registry identity recorded); add it if it is cheap and log the choice.

## Fallbacks

No complexity gate. Departures that need the scope reassessment: changing the helm's identity recording, rewriting
diagnostics beyond the two in scope, or changing the lifetime rules of other diagnostics.

Decisions to log: the final wording and its capitalization, how "runs this helm's version" is decided, how values are
escaped, how a change of both destination and identity is worded, and whether the helm-side premise test was added.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-host-changed-update-warning-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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
`scode-model-routing` at any point; the user chose this mode explicitly, so if galaxy-brain is already active when you
start, this file is the user's express instruction to stop it.

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
  `plan/host-changed-update-warning/<nn>-<short-name>` (for example
  `plan/host-changed-update-warning/01-first-identity`).
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
