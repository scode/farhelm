# Execute four triage outcomes: show and log text from hosts with unsafe characters escaped

Written against main at 814a1129 on 2026-10-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out four triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code`, in one draft PR. Their
headings, which are also the feedback file names under `review_feedback_queue/`:

- `status-badges-render-unescaped-host-supplied-text.md`
- `sidebar-directories-visually-disguise-sessions-actual-folder.md`
- `malformed-supervisor-messages-inject-terminal-controls-into.md`
- `remote-attach-errors-inject-terminal-controls-into.md`

All four are about text that comes from a host's supervisor and is not trusted. Farhelm's rule is that such text is
shown to the user with invisible and direction-changing characters made visible, and logged with control characters
neutralized, so a misbehaving host cannot disguise what the user sees or repaint the operator's terminal. Two places in
the GUI break that rule (a session's status badge and the folder shown in the sidebar), and two places in the helm's log
do (a malformed message from a supervisor, and a refused terminal attach). Each fix routes the text through the escaping
helper that the surrounding code already uses.

Each ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for
that outcome. Read the entries and the four feedback files before starting. Precedent for the GUI side: the ledger entry
`session-header-raw-peer-text.md`. Where this file and a ledger entry seem to disagree, the ledger entry wins and the
disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- Tests cover the GUI escaping (a zero-width or direction-override character in a status detail and in a sidebar
  folder). The two helm log lines need no new test (see the outline).
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, adds a changelog fragment, and passed the review gate.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decision (2026-10-10):** a screening of the highest-priority review findings picked out the ones
that are clearly bugs with straightforward fixes, and the user decided: "given the criteria, let's just triage them all
for fixes unless you have a reason to disagree. every one should have a complexity gate so that if anything turns out to
be more complicated than your assessment, it bounces back to me." The criteria were: clearly a bug, no judgment needed;
and fixing it is straightforward, adding no significant complexity or scope. The decisions are recorded in root
`TRIAGE_OUTCOMES.md`, one entry per feedback file, each with its own complexity gate.

**The user's plan-time decisions (2026-10-10):** "group them together into a reasonable number of PRs that are
reasonably related or similar so we don't have 18 distinct plans with tiny amounts of work". Asked to confirm, the user
chose five plans with one PR each, so this plan's outcomes share one PR, overriding root `AGENTS.md`'s
one-PR-per-outcome rule for these outcomes only. When an item trips its complexity gate: drop it from the PR and ship
the rest (see Complexity gate below). Review gate: gpt-6.1-sol at high effort. No-workhorse mode, which
`plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main by a fresh-context
planning review.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Agent scratch space; the rule against tests that modify the test process's
own environment variables), `plans/AGENTS.md` (Executing one plan), `review_feedback_queue/AGENTS.md` (Lifecycle), and
`.agents/test-authoring.md` for test changes.

## Outline

Line numbers drift; find the code by name. Verified on main at 814a1129.

- Status badge: `crates/farhelm-ui/src/status.rs`, `status_badge`, appends the exit annotation and the error detail
  verbatim. Pass both through `crate::peer::display_peer` where they are appended, so the visible text and the
  `data-tooltip` both get the escaped form. The compact sidebar tooltip in `crates/farhelm-ui/src/list/row.rs` already
  escapes the same text; escaping twice is harmless, because the escaped form contains no characters the escaper
  changes. Extend the existing `status_badge` tests (search `status_badge_matches_text_and_class_for_each_status`) with
  a detail holding a zero-width or direction-override character.
- Sidebar folder: `crates/farhelm-ui/src/list/row.rs` computes `cwd_shown = abbreviate_home(&session.cwd)` and renders
  `"{cwd_shown}"` raw inside `span { class: "session-cwd-text", dir: "ltr", ... }`, while its tooltip already uses
  `display_peer`. Render `display_peer(&cwd_shown)` there (escape the abbreviated form, not `session.cwd`, so the `~`
  abbreviation survives). An empty folder will show `(empty)`, as the same row's tooltip already does; no special case.
  Add a test in the style of the neighbouring row tests. The host name rendered raw nearby is not part of this finding;
  leave it.
- Helm log, malformed message: `crates/farhelm-helm/src/client.rs`,
  `warn!(error = %e, "invalid frame from supervisor")`. The error can embed the peer's unknown message type verbatim
  (serde's "unknown variant" text), and the log formatter does not escape fields. Log
  `error = %crate::manager::peer_text(&format!("{e:#}"))`, the helper `manager.rs` already uses for peer text
  (`pub fn peer_text`).
- Helm log, refused attach: `crates/farhelm-helm/src/terminal.rs`,
  `error!(error = %e, "terminal websocket ended with error")`, where `e` can carry a supervisor's refusal message. Same
  change. Escaping errors that originate in the helm itself is harmless.
- Tests for the helm lines: `farhelm-helm` has no log-capture facility, so these two lines get no new test; the helper
  itself is covered by `peer_text_bounds_and_escapes`. Do not build log-capture machinery for this.
- Expected side effect: an empty or whitespace-only status detail now shows as `display_peer`'s placeholder (`(empty)`
  or a whitespace-only note) instead of nothing. That is consistent with the rest of the GUI and not a regression; say
  so in the review prompt.
- Changelog: `fix:` with `kind: fixed`, written for someone running Farhelm: text a host sends that appears in session
  status badges and the sidebar's folder line is now shown with hidden and direction-changing characters made visible.
  The log change can be mentioned briefly or left to the PR description.

The complexity gate for these four is a one- or two-line change per site plus tests. A new escaping helper, a change to
the log formatter or subscriber setup, or changes beyond these four sites and their tests is a gate trip.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` (or
at least on the two touched crates), and focused nextest selections for the touched `farhelm-ui` and `farhelm-helm`
tests through `scripts/record-test-run.py` (see `docs/test-run-evidence.md`). Run
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` and apply `.agents/test-authoring.md`.
`dprint check` on the changed Markdown. No browser run is needed unless the GUI change turns out to need more than text
escaping.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-untrusted-text-escaping-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution fields on the PR) rather than starting over. If it does not exist, this is a
fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/AGENTS.md` (Executing one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/untrusted-text-escaping/01-escape-peer-text`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); `kind: none` with a one-line reason is right when nothing changes for
  someone running Farhelm. Validate with `python3 releasing/check-changelog.py format`.
- For every outcome the PR carries out, it removes the feedback file under `review_feedback_queue/` and its line in
  `review_feedback_queue/INDEX.md`, and updates that outcome's `TRIAGE_OUTCOMES.md` Execution field to `complete` with
  the jj change ID, bookmark and PR URL. Record the change ID and bookmark before creating the PR, then add the URL to
  the same change and push again; no separate bookkeeping PR. An outcome dropped by its complexity gate is handled as
  Complexity gate says instead.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Complexity gate

Every outcome in this plan carries the user's complexity gate, stated in its ledger entry and in the outline above: the
fix should stay about as small as assessed at triage. If one turns out to need significantly more (a new mechanism,
cross-component plumbing, a change well outside the files the outline names, or a product or design decision), do not
grow it and do not block the whole plan for it. Leave that outcome out of the PR, keep its feedback file and index line,
and in the same PR set its `TRIAGE_OUTCOMES.md` Execution field to `pending`, waiting on the user, followed by what you
found and the realistic options with a recommendation (see `installer-startup-prune.md` in the ledger for the shape).
Name it among the open questions in the plan's report. The rest of the PR ships. Block per Unattended fallback only when
every outcome in this plan was dropped, since then there is no PR to deliver. Code that turned out to be already fixed
on main is not a gate trip: record the outcome as `discard` with "already fixed" and the fixing commit, per root
`AGENTS.md`, and remove its queue item.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria: each outcome's `TRIAGE_OUTCOMES.md` entry, the outline above, and
the user's decisions. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim, as root `AGENTS.md` requires. Also ask the reviewer to check that every changed site escapes text from a host
and nothing else changes in what users see for ordinary text. Where you disagree with a finding, decide on the merits
and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new helper module, a
change to a shared formatter or protocol, edits outside the files the outline names beyond tests and bookkeeping), and
whenever the same component has needed repeated corrective review rounds, run a fresh-context review through
galaxy-brain with this charter, supplying the ledger entries, the user's decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A departure that
the review does not find necessary and small is a complexity-gate trip, handled as above.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular how the empty-folder case renders, whether the helm log lines got tests, the
Conventional Commit type and changelog kind, every outcome dropped by its complexity gate and why, and every review
finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is the complexity gate above. If current code or specs have
moved so that a recorded decision no longer applies, treat it like a gate trip for that outcome, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself. Block per `plans/AGENTS.md` (Executing one plan, step 10)
only when no outcome is left to ship or the shared PR itself cannot proceed without the user.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate, meets the acceptance criteria above, and passed the review gate, with every outcome's
`TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out outcomes removed. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
