# Fall back to the main terminal when a remembered shell tab cannot attach

Written against main at 8007a789 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Show the main terminal when a remembered shell tab cannot attach.** Reopening a session can leave it stuck
> reconnecting to a remembered shell tab whose terminal the supervisor cannot read. Fall back to the main terminal after
> an explicit initial refusal, even if the shell tab remains listed. The terminal stream currently uses the same detach
> code for that refusal and for some dropped supervisor connections. Give the desktop app a way to tell them apart so a
> temporary outage keeps retrying. Route pending upload notices to the terminal the app ends up showing, and keep the
> shell tab available for a deliberate selection.

In product terms: distinguish an explicit supervisor refusal to attach a shell tab (`ErrorOrigin::SupervisorReply` +
`ErrorKind::Unavailable`) from a temporary supervisor disconnect or unreachable host on the helm ↔ desktop terminal
stream (no supervisor protocol version bump needed). When reopening a session tries to restore a remembered shell tab
and gets that explicit initial refusal (or `TabClosed`), fall back to the healthy main terminal — routing any pending
carried upload notices to the terminal the app actually shows — while keeping temporary supervisor outages retrying.
Keep the shell tab listed in the tab bar, and if the user deliberately selects it, stop auto-retrying and show
`"Detached: <supervisor reason>"` with a **Reconnect** button.

### Where it stands (from planning research at main 8007a789; line numbers approximate)

- **Helm terminal WebSocket (`crates/farhelm-proto/src/terminal_socket.rs:41-47`,
  `crates/farhelm-helm/src/terminal.rs:502-523, 2361-2383`):**
  - `SupervisorError` (`crates/farhelm-helm/src/client.rs:296-375`) already distinguishes `ErrorOrigin::SupervisorReply`
    (explicit refusal from the supervisor's `host_attach_refusal`,
    `crates/farhelm-supervisor/src/service/handlers.rs:1558-1575`) from `ErrorOrigin::Helm` (unreachable host) and
    `SupervisorTransportError` (dropped supervisor connection).
  - `serve_term` (`terminal.rs:502-523`) currently maps only `q.tab.is_some() && kind == NotFound` to
    `DetachCode::TabClosed`, and everything else to `DetachCode::Other`.
  - Add an additive wire field on `TerminalServerMessage::Detached`:
    `#[serde(default, skip_serializing_if = "std::ops::Not::not")] pub tab_unavailable: bool`. Leave `DetachCode`,
    `ControlMsg::Detached` (supervisor ↔ helm, `PROTOCOL_VERSION`), and `crate::client::Detach` untouched so older
    desktop builds still parse `code: "other"` cleanly. Set `tab_unavailable: true` strictly in `serve_term`'s initial
    attach-failure arm when `q.tab.is_some()` and attach failed with `SupervisorError` having
    `origin == ErrorOrigin::SupervisorReply && kind == ErrorKind::Unavailable`.
  - Split/update `only_a_missing_tab_is_reported_as_tab_closed` (`terminal.rs:2361-2383`) so it verifies helm-side
    unreachable/dropped connection sends `code: "other", tab_unavailable: false` while supervisor-replied `Unavailable`
    on a tab sends `tab_unavailable: true`.
- **Helm client & fake (`crates/farhelm-helm-client/src/terminal.rs:383-388`, `fake/terminal.rs:227-238, 540, 727`):**
  - Add `tab_unavailable: bool` directly to `StreamState::Detached` and update `detach_stops(code, tab_unavailable)` so
    `tab_unavailable: true` stops auto-retry (while `code: Other, tab_unavailable: false` keeps retrying in
    `StreamState::Reconnecting` for temporary outages). Update `FakeTerminal` (e.g. `detach_unavailable` or
    `tab_unavailable: bool`) so `crates/farhelm-helm-client/src/terminal/tests/parity.rs` and all GUI terminal tests can
    drive it. Update the 1-line destructuring pattern at `crates/gui/farhelm-gui/src/terminal/attached.rs:865` (`..`) in
    PR 1 so the workspace compiles; `AttachedTerminal::status_line()` already renders `"Detached: <reason>"` +
    **Reconnect** for `code: Some(DetachCode::Other)` with zero logic change.
- **Desktop GUI (`crates/gui/farhelm-gui/src/terminal/set.rs:251-261, 287-366, 524-631`, `attached.rs`,
  `attachments.rs:394-442, 757-825`):**
  - Reuse `TerminalSet.restore`: replace `restore: Option<Mark>` (`set.rs:302`) with a 2-variant `Option<Restore>`
    (`WaitingForRow(Mark)` / `Attaching(String)`). That single field governs both automatic-restore fallback eligibility
    AND the existing `if self.restore.is_some() { return; }` gate in `take_carried` (`set.rs:623`), while
    `post_departed_uploads` (`attachments.rs:394-442`) continues promoting carried uploads to window notices if the user
    leaves the session before initial attach settles (avoiding any new un-retiring transfer API on `Attachments`).
  - Observe tab `AttachedTerminal` entities via a `Subscription` stored on `Tab` (`set.rs:251-261`, dropped
    automatically when a tab is removed), using a tiny `pub(super)` stream-state helper on `AttachedTerminal`:
    - When `follow_tabs` selects remembered tab `id`, transition `restore` to `Some(Restore::Attaching(id))`.
    - Keep `Restore::Attaching(id)` pending across transient `Reconnecting` states (temporary supervisor outages keep
      retrying without clearing fallback eligibility or stranding notices).
    - On `Attached`: clear `restore = None` and call `take_carried(cx)` (delivers carried notices to the shell tab).
    - On `Detached { code, tab_unavailable, .. }` with `tab_unavailable || code == Some(DetachCode::TabClosed)` before
      ever reaching `Attached`: set `self.chosen = None` (leaving `ShownTerminals` intact, matching
      `a_missing_tab_falls_back_to_agent_without_erasing_the_choice`), clear `restore = None`, call `take_carried(cx)`
      (delivers carried notices to `self.agent`), and `cx.notify()` (updating focus home via
      `session_pane.rs:306, 505`).
    - On deliberate `choose(...)`, `opened`, or tab disappearance: clear `restore = None` and call `take_carried(cx)`.
- **Specs & tests:** `SPEC.md:2358-2361`, `SPEC_impl.md:2850-2866`; tests in `farhelm-helm/src/terminal.rs`,
  `crates/farhelm-helm-client/src/terminal/tests.rs` + `crates/farhelm-helm-client/src/terminal/tests/parity.rs`, and
  `crates/gui/farhelm-gui/src/terminal/set/tests.rs` + `attached/tests.rs`.

### Acceptance criteria

1. Helm ↔ desktop `TerminalServerMessage::Detached` distinguishes an explicit supervisor `Unavailable` refusal on a
   shell tab from a dropped supervisor connection or unreachable host, without bumping `PROTOCOL_VERSION`.
2. Temporary supervisor disconnects / unreachable hosts keep retrying via `Reconnecting` without triggering fallback
   (and if a subsequent reconnect receives an explicit initial refusal before ever attaching, fallback still triggers).
3. Reopening a session whose remembered shell tab receives an explicit initial refusal (or `TabClosed`) falls back to
   the main terminal even while the shell tab stays listed, keeps the shell tab available in the tab bar, and does not
   erase `ShownTerminals` unless the user explicitly clicks the main terminal.
4. Deliberately clicking that unavailable shell tab stops auto-retrying and displays `"Detached: <supervisor reason>"`
   with a **Reconnect** button.
5. Pending/carried upload notices route to the terminal the app ends up showing (main terminal after fallback, or shell
   tab after successful attach; still promoted to window notices if the user leaves before attach settles).
6. Tests cover helm, helm-client (`parity.rs` + `FakeTerminal`), and GUI fallback, temporary outage retry, deliberate
   selection, and carried upload notice routing; `SPEC_impl.md` is updated; and the TODO.md entry is removed in the last
   PR.

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- Deliberate selection of an unavailable shell tab stops auto-retrying and shows `"Detached: <supervisor reason>"` with
  a **Reconnect** button.
- Wire scope is helm-to-desktop only (no supervisor protocol version bump; fall back on any explicit supervisor
  `Unavailable` refusal for a remembered tab).
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: `No release note needed.` (no highlight; folded full list only).

**Binding repository constraints:**

- `SPEC.md:2358-2361` and `SPEC_impl.md:2850-2866`.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and "Instructions by path".

**Planner proposals:**

- Use an additive `#[serde(default, skip_serializing_if = "std::ops::Not::not")] pub tab_unavailable: bool` field on
  `TerminalServerMessage::Detached` and `StreamState::Detached` rather than mutating `DetachCode` or introducing a
  second detach enum.
- Reuse `TerminalSet.restore` as a 2-variant `Option<Restore>` (`WaitingForRow(Mark)` / `Attaching(String)`) so
  automatic-restore fallback and `take_carried` gating share one state machine.
- Also fall back on an initial `TabClosed` for a remembered tab (handles a stale tab roster at reopen cleanly).
- Slice into 2 PRs: (1) proto + helm + helm-client (`FakeTerminal`, `parity.rs`) signal & 1-line `attached.rs` pattern
  update, (2) GUI `TerminalSet` fallback + carried upload notice routing + tests + `SPEC_impl.md` + TODO removal.

## Outline

1. `plan/fallback-unattachable-remembered-tab/01-terminal-detach-unavailable-signal`: add `tab_unavailable` to
   `TerminalServerMessage::Detached` (`farhelm-proto`), set it in `farhelm-helm/src/terminal.rs` only on explicit
   supervisor-replied `ErrorKind::Unavailable` for a tab attach (leaving helm-side unreachable / transport drops with
   `tab_unavailable: false`), surface `tab_unavailable: bool` on `StreamState::Detached` in `farhelm-helm-client`
   (updating `FakeTerminal` and stopping auto-retry when `tab_unavailable` is true), update the `attached.rs:865`
   pattern (`..`), and add proto/helm/helm-client (`parity.rs`) tests.
   `fix: mark explicit shell-tab attach refusals on terminal detach` (with `kind: fixed` changelog fragment).
2. `plan/fallback-unattachable-remembered-tab/02-gui-remembered-tab-fallback`: extend `TerminalSet.restore` to
   `WaitingForRow(Mark)` / `Attaching(String)` and observe tab `AttachedTerminal` entities in
   `crates/gui/farhelm-gui/src/terminal/set.rs`, fall back to the main terminal on explicit initial refusal (or
   `TabClosed`) without erasing `ShownTerminals`, keep deliberate clicks showing `"Detached: <reason>"` + Reconnect,
   route pending/carried upload notices to the terminal shown, update `SPEC_impl.md`, add GUI tests, and remove the
   TODO.md entry ("Show the main terminal when a remembered shell tab cannot attach").
   `fix: fall back to main terminal when remembered shell tab fails` (amends PR 1's changelog fragment).

## Fallbacks

- None expected; escalate via `queue block` only if a repository invariant conflicts with the settled behavior above.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-fallback-unattachable-remembered-tab-log.md` in the parent directory of the checkout you run in, derived
as that section says, and the watchdog's status file is
`farhelm-plan-fallback-unattachable-remembered-tab-watchdog.json` beside it. Resolve both to absolute paths before you
start; this file does not create either. Scratch files go in the agent scratch directory that `AGENTS.md`, "Agent
scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run targeted tests in `farhelm-proto`, `farhelm-helm` (`terminal`), `farhelm-helm-client` (`terminal`), and
`farhelm-gui` (`terminal::set::tests`, `terminal::attached::tests`, `terminal::attachments`),
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `dprint check` on changed Markdown, and
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it. Apply `.agents/test-authoring.md` to
every test you add or change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

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
- The last PR of the stack removes the TODO.md entry this plan covers ("Show the main terminal when a remembered shell
  tab cannot attach"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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
