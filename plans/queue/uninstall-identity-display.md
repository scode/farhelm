# Hosts: stop quoting identities ambiguously in uninstall refusals

Written against main at aa245430 on 2026-10-24. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

When a host's Uninstall is refused because its identity changed, the refusal sentence quotes the recorded and reported
installation identities with the helm's diagnostic escaping (`peer_text`, `crates/farhelm-helm/src/manager.rs` around
718), which is Rust's `{:?}` quoting. That keeps ordinary spaces and printable non-ASCII letters as they are, and the
GUI's host refusal dialog collapses runs of spaces, so two different identities can look identical. SPEC.md (around line
3285, confirmed 2026-10-10) says identity labels the user compares "also show spaces visibly, so two different
identities never look alike". The TODO.md entry (bucket "Near term"), verbatim:

> **Show changed host identities unambiguously in uninstall refusals.** When Uninstall is refused because a host's
> identity changed, the message shows the identities without making spaces visible, so two different identities can look
> the same; the host list and the adopt prompt already spell them unambiguously. Show them the way the host list does,
> or drop them and point to the host list. Complexity gate: fix it only if the fix is simple; otherwise leave it. From
> the review of `uninstall-refusal-wording` on 2026-10-23.

The maintainer chose to drop the identities and point to the host list.

Where it lives (main at aa245430, line numbers approximate), both in
`crates/farhelm-helm/src/provisioning/uninstall_refusal.rs`:

- `unusable_host`, its `HostState::IdentityMismatch` arm (around 55-63): the host is not connected because its identity
  no longer matches the record; the dialog offers Remove, and the sentence says to choose adopt in the host's menu.
- `changed_identity` (around 125-138): a connected host's fresh check reports a different identity than the record;
  plain refusal, remedy Retry and then follow the host list.

The refusal is shown only in the GUI's host refusal dialog (`HostRefusalDialog`, `crates/farhelm-ui/src/hosts.rs` around
145-222), which the web UI and the desktop app share. `farhelm uninstall` on the command line is the local machine's own
uninstall and never reaches these sentences.

### Acceptance criteria

- Neither refusal sentence contains an identity value, in any spelling, nor the "no identity" placeholder. Each instead
  points the user to the host list for the identities.
- The mismatch refusal keeps its remedy (adopt in the host's menu, or Remove, which the dialog offers) and says the host
  list shows both the recorded and the reported identity.
- The changed-identity refusal drops only the two quoted values (for example "the Farhelm answering at its ssh
  destination is not the install this entry has on record") and keeps today's remedy clause unchanged ("Choose retry in
  the host's menu and follow what the host list then says, then choose uninstall again"), without claiming what the host
  list will show. That clause is accurate in all three cases the refusal covers: recorded A and reported B (Retry shows
  a mismatch with both), recorded A and reported none (Retry shows the host as unverified, with only the recorded
  identity), and none recorded with B reported (Retry records B and connects, or shows a duplicate).
- Wording follows SPEC.md's uninstall refusal rules (around 167-169: a changed identity "also explains the remedy and
  offers Remove") and the product-terms rules in root `AGENTS.md`, "Talking to the user" (in UI text: no internal
  names).
- Parameters that become unused are removed, with their callers in `crates/farhelm-helm/src/provisioning/service.rs`
  (around 1361-1364 for `changed_identity`) updated.
- Tests: the `uninstall_refusal.rs` unit tests that assert the quoted identities
  (`identity_refusals_keep_their_distinct_remedies_and_escape_peer_text` around 178-205, and
  `neighbouring_refusals_explain_connection_and_identity_recovery` around 257-265) and the provisioning tests in
  `crates/farhelm-helm/src/provisioning.rs` that assert `no identity` / `no identity on record` / `someone-else` (around
  4941 and 4980-5017) are rewritten to the new contract. Only two places still receive identities and so can show their
  absence: the mismatch arm of `unusable_host`, and the provisioning-level test that drives identities through the
  service. In each, one identity with a doubled space or a lookalike letter is enough to assert that neither the value
  nor its quoted form appears while the host-list pointer and the remedy do; once `changed_identity` takes no
  identities, do not add a parameter just to test its absence. No browser test is needed; the dialog renders the string
  as given.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: fixed`): the
  identity text shipped in v0.31.0, so this is a new fragment, not an edit of an earlier one.
- The PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-10-24, on the planning page:

- **Drop the identities and point to the host list**, chosen over spelling them the way the host list does (which would
  have moved the UI's `display_identity` spelling into `farhelm-proto` so the helm could use it, about 40-60 lines
  across three crates). The host list row beside the refusal already shows both identities with the strict spelling, and
  the adopt item names the reported one.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

Stated to the maintainer on the planning page, without objection: the fix passes the entry's complexity gate. (The page
also said the changed-identity sentence would promise both identities after Retry; the planning review showed that is
false in two of its three cases, so the plan keeps today's remedy clause instead, which serves the same choice.)

## Requirement sources

**The user's request:** the TODO.md entry quoted above, and the decisions above.

**Binding repository constraints:** SPEC.md's host uninstall rules (around 160-175) and identity display rule (around
3285-3289); root `AGENTS.md`, "Talking to the user", "Tests and validation", "Conventional Commits", "Changelog
fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md` for the test changes.

**Planner proposals** (from a read of main at aa245430; change them if the code says otherwise, logged as a DECISION):

- Out of scope: the setup/update attach refusal (`attach_refusal` in `provisioning/service.rs` around 204-243) quotes
  identities with `peer_text` the same way. It is not an uninstall refusal and the maintainer chose the drop option for
  this entry; mention it in the report's follow-ups rather than changing it.

## Outline

One PR, `plan/uninstall-identity-display/01-drop-identities`: the two sentences, their parameters and callers, the test
rewrites, the changelog fragment and the TODO.md removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-helm`, and through `python3 scripts/record-test-run.py` the narrow nextest filter over the uninstall refusal
and provisioning uninstall tests, for example
`cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::uninstall_refusal::) | test(provisioning::tests::uninstall_)'`.

## Fallbacks

Examples of departures that need the scope reassessment: spelling identities after all, a shared identity-spelling
helper, changing the host refusal dialog or its CSS, or changing the attach refusal.

Decisions to log: the final wording of both sentences, every test changed and why, and every review finding you decided
not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-uninstall-identity-display-log.md` in the parent directory of the checkout you run in, derived as that
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/uninstall-identity-display/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
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
