# Exited sessions whose terminal is gone offer Restart and Replace

Written against main at d480779c on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **An exited session without a terminal keeps "reconnecting" forever.** When a session's terminal host has died (for
> example after a container was stopped and started from outside Farhelm), the session correctly reads "Exited", but its
> pane stays on "Session unavailable — reconnecting" and retries every 30 seconds. The supervisor refuses the attach
> ("has no terminal on this host"), the helm reports that to the app as an ordinary disconnect, and the app keeps
> retrying a connection that cannot come back. The browser app behaved the same. Agreed fix (2026-11-16): add a "has a
> terminal" fact to the session row (protocol and supervisor), and have the pane show a no-terminal card with Restart
> and Replace instead of mounting a terminal, reusing the interrupted card's path. This widens SPEC's rule for sessions
> with no terminal after a reboot. Rejected: a new disconnect reason that stops the retrying, because the same refusal
> may be transient while a Restart relaunches the terminal.

The fix is the one the maintainer agreed on 2026-11-16 and the entry records: a "has a terminal" fact on the session
row, carried by the protocol and set by the supervisor, and a no-terminal card in the desktop app's session pane with
Restart and Replace, reusing the interrupted card's path, instead of mounting a terminal that can never attach.

### Acceptance criteria

1. Every session a supervisor reports (the session list and every single-session reply) says whether the session's agent
   terminal is missing on that host. The supervisor sets it from whether it holds an agent terminal for the session and
   from nothing else, in every place that builds a session reply that can carry it: `entry_info` for the list and most
   replies, and the Restart reply (`publish_relaunched`, which knows whether it published a new terminal only after its
   reply's fields are first filled in). A Restart that starts a new terminal clears it.
2. The field's absent value means "has a terminal": a bool such as `terminal_missing`, defaulting to false and omitted
   when false, the same pattern as `screen_yolo` and the record's other bools. So JSON the helm cached before the
   upgrade, create replies and held rows need no change, and only the replies of a terminal-less session carry it.
3. This is a helm-supervisor protocol change with a protocol version bump (the maintainer's choice, and what the
   protocol rules at the top of `crates/farhelm-proto/src/lib.rs` require for a field that changes what the receiver
   does; `PROTOCOL_VERSION` is in that file). The helm passes the fact through to the app unchanged, including through
   its session cache and the merges it makes when a mutation's reply arrives.
4. In the desktop app, a session on a reachable host that is not retired and has no agent terminal (whatever its status:
   Exited, or unknown after a Restart whose failed start could not be confirmed) opens on a card in place of the
   terminal, with the interrupted card's layout and focus behavior, and does not mount a terminal or retry an attach.
   The card offers what the interrupted card offers for the same session: Restart (primary) when the session can be
   restarted, Replace, Replace with for a launch that needs it, Replace's inline question, and the Restart wait line.
   Its text reuses the interrupted card's text, replacing only its opening clause (the one that says the session was
   interrupted) with one generic sentence close to: "This session's terminal is gone: it ended outside Farhelm (for
   example when its container was stopped), never started, or could not be found on this host." The interrupted card's
   own Restart, Replace and "cannot resume, because ..." clauses follow it as they do there, so the card still says why
   a session cannot resume. Log the final text as a DECISION.
5. The session's other terminal tabs are hidden while the card shows, as the interrupted card hides them.
6. The "just restarted" mark that keeps the card from flashing back after a Restart follows the widened rule too
   (`forget_settled` in `session_pane/card.rs` drops it today as soon as a row is not Interrupted, which would drop it
   at once for a terminal-less Exited session).
7. An Interrupted session still shows the interrupted card with its own text; an unreachable host still shows the
   unavailable card; a retired session still shows the retired card.
8. SPEC.md's rule for a session with no terminal (around its lines 2200 to 2201 at planning time, "after a reboot")
   widens to any session whose terminal no longer exists on a reachable host, and says the pane offers Restart and
   Replace; SPEC.md's reconnect-card paragraph says that card is not used for such a session; SPEC_impl.md's description
   of how the pane follows the store for its card mentions the no-terminal fact. The doc comment on the supervisor's
   attach refusal that records this bug (`service/terminals.rs`, around lines 154 to 162) is updated.
9. Tests: the supervisor sets the fact for a session with no terminal (the existing test hook that forgets a session's
   agent terminal does this) and clears it after a Restart; a protocol round-trip test that an absent field decodes as
   "has a terminal"; card and pane tests that a terminal-less exited session opens on the card and asks the helm for no
   attach, that the fact clearing mounts a terminal, that the restarted mark survives until it clears, and that the
   other cards are unchanged; two screenshot goldens for the no-terminal card, mirroring the interrupted card's two (can
   resume, and not captured). An end-to-end test in `crates/farhelm/tests/e2e/host_terminals.rs` (which already uses the
   forgetting hook) if it fits.
10. A changelog fragment per root `AGENTS.md`, "Changelog fragments" (a protocol bump alone is `kind: none`, but this
    changes what users see, so the app PR's fragment is `fixed`), and the last code PR removes this plan's TODO.md
    entry.

## Decisions already made

Settled with the maintainer on the 2026-11-18 planning page, which showed a mockup of the card:

- **Bump the protocol version.** The planner recommended an optional field without a bump (an older host would simply
  keep today's behavior, with the session-notifications field as precedent); the maintainer chose the bump.
- **Hide the other tabs while the card shows,** as the interrupted card does. Rejected: keeping the tab strip.
- **One generic sentence.** Rejected: a reason per case, which would need the host to report a reason vocabulary.
- **Same styling as the interrupted card,** text changed only (planner choice, shown on the mockup).
- **Release note:** none needed (see the section at the end).
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending**, with no dependency on another plan. The pending plan `drawn-frame-actions` edits the
  interrupted card's Restart and Replace (`session_pane/card.rs`); whichever lands second rebases across it.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry (its agreed fix and its rejected alternative) and the
Decisions above.

**Binding repository constraints:** SPEC.md's no-terminal rule, the interrupted card and the reconnect card; the
protocol rules and `PROTOCOL_VERSION` at the top of `crates/farhelm-proto/src/lib.rs`; root `AGENTS.md`'s rules on
fragments for a protocol bump; root `AGENTS.md`, "Code rules", "Changelog fragments", "Conventional Commits", "Tests and
validation"; `.agents/test-authoring.md`.

**Planner proposals** (from reading the code at planning time; follow the code where it differs, logged as a DECISION):

- The supervisor keeps an optional agent terminal on each session (`service/core.rs`). It is empty only when a
  supervisor start found no usable terminal for a stored session, when an update ended an old tmux terminal, or when a
  Restart whose failed start cannot be confirmed as ended leaves the session with none. A terminal host that may still
  run but cannot be read keeps a handle that answers "unavailable", never "no terminal" (SPEC_impl.md), so that case is
  out of scope and keeps today's behavior.
- `entry_info` (`service/status.rs`) builds the list and most single-session replies; the Restart reply is a separate
  literal in `publish_relaunched` (`service/core.rs`), where the fact comes from whether `publishable(Some(terminal))`
  publishes a terminal (it can be none after an orderly stop). Create and replay replies, and the held rows listed
  during a restart, are also built outside `entry_info`; with the field absent meaning "has a terminal" they need no
  change.
- The wire record is `SessionInfo` (`crates/farhelm-proto/src/lib.rs`); the app reads it flattened in its session row
  (`ui_channel.rs`); the helm caches it as JSON and, on a mutation's reply, merges only status and timestamps
  (`session_cache.rs`), so a new field passes through. About 200 places spell out a `SessionInfo` field by field, mostly
  tests; each needs the new field.
- The app's interrupted card rule is in `session_pane/card.rs` (reachable, Interrupted, not retired, not just
  restarted); widen it to (Interrupted or no terminal), have `forget_settled` use the widened rule, and pass the lead
  clause into the card's existing `text()` composition. The pane mounts no terminals while the card shows
  (`session_pane.rs`), and `card::Follow` tells it when that changes, so hidden tabs come for free.

## Outline

1. `plan/exited-session-no-terminal/01-report-missing-terminal`, `fix:`: the protocol field and version bump, the
   supervisor setting it in `entry_info`, the helm passing it through, the field added everywhere a `SessionInfo` is
   spelled out, and the supervisor and protocol tests. Its fragment is `kind: none` (a protocol bump alone) unless it
   changes something users see.
2. `plan/exited-session-no-terminal/02-no-terminal-card`, `fix:`: the app's card for a terminal-less session, its text,
   card and pane tests, goldens, the SPEC.md and SPEC_impl.md edits, the refusal's doc comment, the `fixed` fragment,
   and the TODO.md entry removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
the touched crates, the proto, supervisor, helm and GUI unit tests that cover session replies, the cache and the session
pane through `python3 scripts/record-test-run.py`, the end-to-end test if added, the GPUI goldens, and
`python3 releasing/check-changelog.py format`. A protocol bump also runs whatever `.agents/validation.md` names for one
(version parity, say).

## Fallbacks

- If another reply path turns out to report a terminal-less session without going through `entry_info` or
  `publish_relaunched`, set the fact there from the same terminal handle, rather than adding a second notion of "has a
  terminal".
- If hiding the other tabs strands a tab that still runs a user process with no way to reach it, keep the maintainer's
  decision and say so in the report's things you should know.
- If the fact turns out to need persisting in the helm's database (it should not: it rides on the cached JSON), that is
  a departure: run the scope reassessment.

Decisions to log: the field's name and wire shape, the new protocol version, where the Restart reply gets the fact, the
card's final wording, every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-exited-session-no-terminal-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-exited-session-no-terminal-watchdog.json` beside it.
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
  `plan/exited-session-no-terminal/<nn>-<short-name>`, as the outline names them.
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
