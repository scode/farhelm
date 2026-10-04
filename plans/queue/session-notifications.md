# Session notifications: a bell on the sidebar row when session tracking breaks

Written against main at 7444371d on 2026-10-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file directs a change to them (M1).

This plan runs after `plans/queue/launch-representation.md` and `plans/queue/hover-help.md` (its `INDEX.md` line says
so). The launch redesign rewrites hook injection (to placeholder expansion with `{farhelm_args}`), moves reporter
settings into the launch specification, and replaces "Restart offers a fresh launch" with "Restart unavailable, with a
reason", so several of the warnings below move or change meaning. Hover help gives every control a fast tooltip through
a `data-tooltip` attribute and a browser test that fails on a control without one; the bell this plan adds must follow
that. Names below come from main at 7444371d and some will have moved. Read what both plans landed before starting.

`plans/queue/hook-report-files.md`, planned 2026-10-04, also touches this plan's ground and is meant to land first; if
it has, read what it landed before starting. It replaces the hook's socket round trip with report files the supervisor
applies on its reconciliation pass, so conversation identity reports are no longer refused at a socket handler
(`service/handlers.rs`) or answered to the hook: refusals happen when the supervisor reads a report file, and the
65-second tripwire may see a report up to one pass later. Classify refusal sites (below) where that plan left them; the
"transient errors whose own log line says a later report may recover" may no longer exist in that form. If it has not
landed when you start, the names below still hold.

## The goal

Farhelm notices several ways in which it has lost track of a session's agent conversation, which decides whether Restart
can resume that conversation later, and today it only writes them to the supervisor's log, which nobody reads. The user
finds out much later, when Restart cannot resume. The lead example: a session launched with Farhelm's conversation hook
(Claude, Codex) has had input for 65 seconds and no conversation identity report has arrived; the supervisor logs "this
session was launched with a conversation hook but holds no conversation identity" (`report_liveness_tripwire` in
`crates/farhelm-supervisor/src/service/capture.rs`) and nothing else happens.

After this plan, these problems become notifications attached to their session. A session with notifications shows a
bell on its sidebar row; the bell shouts when one is unread; clicking it lists them, and the list can be cleared.

Acceptance criteria:

- The bell, on the sidebar row:
  - It sits between the agent and permission marks and the activity time, in both row densities (normal and compact).
  - It is shown only on rows whose session has at least one notification that has not been cleared. Rows without
    notifications show no bell. Keep the activity time aligned across rows either way (a reserved empty track, or
    whatever the row grid already does for optional slots).
  - With no unread notification it is greyed out, like other quiet row glyphs. With at least one unread notification it
    is unmissable: a very obvious "yelling" color and style (planner proposal: filled, in the app's error red).
  - It has hover text through the hover-help tooltip and an accessible label that includes the unread count.
  - Clicking it opens its popover and does not open the session (the row's own click behavior is unchanged).
- The popover:
  - Visually strongly inspired by the row `⋯` menu popover (same surface, border, shadow, spacing, type), and like it,
    never clipped by the sidebar.
  - Lists the session's notifications, newest first. Each shows how long ago it happened, what happened, and what the
    user can do about it (see the wording rule below). Notifications that are new since the user last opened this
    session's list are obviously distinct from the ones already seen.
  - Has a button that clears all of the session's notifications. After clearing, the bell disappears from the row until
    a new notification arrives.
  - Closing it (clicking away, Escape, the bell again) marks everything it showed as read. There is no "mark unread".
- Storage and lifetime:
  - Notifications persist: they survive supervisor restarts, helm restarts, and reloading the UI.
  - Each session keeps at most its 10 most recent notifications; older ones are dropped. There is no time-based expiry.
  - Read and cleared state survives restarts too, and is the same in every client of a helm, like the per-session "seen"
    state. (The desktop app embeds its own helm and a browser is served by a standalone one; sharing read state between
    different helms is not required.)
  - Deleting a session deletes its notifications.
- Contents, in this first version, are session-tracking problems:
  - an injected-hook launch that has had input for 65 seconds and still holds no conversation identity (the existing
    tripwire);
  - Farhelm could not add its conversation hook to a launch (today an `info!` in `with_hook_argv_using` in
    `crates/farhelm-supervisor/src/service/core.rs`, for reasons such as the user's own `--settings` or a bare `--`).
    After the launch redesign this may survive only for legacy launches; keep it if Replace (or whatever the redesign
    offers) gives the user a way out, otherwise drop it under the agreed fallback;
  - a conversation identity report refused for a reason that indicates a real problem with this session's tracking, and
    only that. Most refusals are the system working and must NOT notify: reports turned away because they came from a
    subagent or from an agent process the session's agent started itself (for example `claude -p` run inside the
    session, which inherits the session's credential), reports made moot by a relaunch or delete that raced them, and
    transient errors whose own log line says a later report may recover. Classify every refusal site
    (`Supervisor::report_conversation` in `service/core.rs`, the doorway refusals in `service/handlers.rs`, the Claude
    vendor refusal in `service/core/vendor/claude.rs`) and log the classification as a DECISION. A planning-time review
    found no refusal of the session's own foreground agent for a non-transient reason; if you confirm that, this item
    records nothing, and the report says so;
  - a resume offer was withdrawn because the agent's own record turned out unavailable or inconsistent
    (`service/core/vendor/codex.rs`, `service/core/vendor/grok.rs`, and the comparable OMP sites in
    `service/core/vendor/omp.rs`: an installed reporter that does not match this build or could not be verified,
    provenance not recorded). Re-derive this list from the code the launch redesign left behind: the point is "session
    tracking is broken for this session", so a site that moved is still in scope, and one that no longer exists is not.
- The same problem notifies at most once per launch of the session's agent (a launch is the session row's existing
  `generation`), so one stuck session cannot fill its 10 slots with copies, and a supervisor restart does not repeat a
  notification already recorded for that launch.
- Every notification tells the user something they can act on (M5), and is true when recorded. The 65-second tripwire
  was built as a log-only diagnostic and tolerates two false firings that a notification must not (M10):
  - The early clock, which matters: its 65 seconds start at the first input delivered to the agent's pane
    (`note_first_input`, called from `service/connection.rs`), and that includes the terminal's automatic replies to the
    agent TUI's own queries (cursor position, colors), as `note_first_input`'s doc admits. A Codex session the user
    opened but has not typed into can therefore trip it, though Codex has nothing to report before a prompt. Start the
    clock instead on the first delivered input to the agent pane that contains a carriage return (a submitted line).
    Before relying on it, verify that the replies xterm.js sends (device attributes, cursor position, OSC color answers,
    focus reports, and any other the vendored version emits) never contain a bare carriage return; if some do, block
    rather than inventing a different heuristic. Update SPEC_impl.md's Runtime state passage, which says "from first
    confirmed agent input".
  - The race, which is minor: a report saved to the database between `capture_now`'s mirror refresh and the tripwire can
    produce one stale firing within milliseconds. Recording already reads the database, so check the session's durable
    captured conversation there and record nothing if one is present. The supervisor's existing log lines stay; the log
    line follows the new clock too.
- SPEC.md no longer says notifications are out of v1; it describes session notifications. SPEC_impl.md describes their
  storage, once-per-launch rule, transport, and read and cleared state. User docs describe the bell, and the hook page
  (`website/src/content/docs/docs/agents/agent-hook-injection.md`, which lists the tripwire line today) says these
  problems now show up as notifications. A changelog fragment exists and the TODO.md entry is removed.

## Decisions already made

Requirement sources are kept apart: the maintainer's words, binding repository rules, and planner proposals.

The maintainer's request and answers:

- M1. The TODO.md entry (Near term, "Notification system"), verbatim: "A way for Farhelm to tell the user about things
  that need their attention, instead of writing them only to a log nobody reads. The first thing to go into it is the
  session-tracking warnings, starting with the supervisor's warning that a session launched with Farhelm's conversation
  hook (Claude, Codex) has had input for a while and no identity report has arrived, which today is only a supervisor
  log line; the user finds out only later, when restart offers a fresh launch instead of Resume. Design TBD." SPEC.md
  says "Notifications (desktop or otherwise) are explicitly out of v1" (Status) and lists "Notifications of any kind"
  under Non-goals for v1; the maintainer said "yes we need to change spec."
- M2. The bell and popover, in the maintainer's words: "in app. i suggest a bell icon to the left of the time indicator
  for each session in the list (we have free space there between the yolo location and the tim). greyed out by default
  turns some very obvious yelling color/style when there's an unread notification. for now, no ability to "mark unread".
  when you click the bell all notifications show, with the ones "new since last time" obviously distinct, but once you
  click away from it they all are considered read. Visual style strongly inspired by the current style of the pop-up
  menus on "..."" Revised afterwards: "lets have it only on rows with notifications, and let's make therebe a button to
  comletely clear notifications once you click to see the list."
- M3. Persistence: "persistent. I suggesting bounded by the last 10. no timestamp based expiry right now."
- M4. Contents: the identity tripwire and the related tracking problems listed above: "knowing that session tracking is
  broken is important." On refusals, after hearing that most are routine: "only some refusal that is due to some reason
  that indicates a problem, not the run of the mill normal refusals that are consistent with a sub agent doing things."
- M5. Wording: the planner may write it, but "limit itself to things the user can actually act upon. for example "check
  the hook log" is nonsense unless we can provide a specific command to run or similar." So each text names a concrete
  action (start the session again with Replace, remove a named flag from the command, run a specific command) or, when
  there is none, says plainly what the user loses (for example that Restart will not be able to resume this
  conversation). Never a vague pointer to logs or diagnostics.
- M6. Scope: "host level and global notifications are out of scope. BUT, we should assume we will eventually add this,
  so when there are cheap ways of just not hard coding an assumption that there is only one type of notification we
  should, but we should not add premature abstraction."
- M7. Runs after `launch-representation.md` and `hover-help.md`.
- M8. Review gate: two fresh-context reviewers per PR, a Claude Opus 5.5 agent at high effort and a gpt-6-astra agent at
  high effort, both with the general review charter (below).
- M9. No-workhorse mode (below).
- M10. The tripwire's clock: the maintainer agreed to start it on the first submitted line (input containing a carriage
  return) instead of any input, so terminal replies cannot start it, and to a cheap re-check of the stored identity at
  recording time for the millisecond race.

Binding repository rules:

- Protocol: `PROTOCOL_VERSION` in `crates/farhelm-proto/src/lib.rs` and its pinning test, and the rules there about what
  needs a bump; the helm and supervisor may run different builds.
- Peer text: anything shown in the UI that a supervisor supplied is peer text and goes through the UI's escaping
  (`display_peer`, `PeerLine`); SPEC.md's trust model treats supervisor content as untrusted.
- Harness-specific behavior: read the module docs of `crates/farhelm-supervisor/src/agent_kind/mod.rs` before adding
  anything per harness; no `kind == X` or wildcard arms over harnesses in shared code (root `AGENTS.md`).
- The supervisor's `SessionsChanged` hint (`hint_sessions_changed`) is how the helm learns a listing changed; polling is
  only the backstop.
- Root `AGENTS.md`: changelog fragment for `feat`; TODO entry removed in the PR that addresses it; "Finishing work" for
  validation; `.agents/test-authoring.md` for test changes; `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`; the docs site's `website/AGENTS.md` and `website/EDITORIAL_RULES.md` before touching it.

Planner proposals (with reasons; change them if the code says otherwise, and log a DECISION):

- P1. Record: in the supervisor, a notification is a per-session sequence number (monotonic per session), a kind, the
  time it was recorded, the launch (`generation`) it belongs to, and its user-facing text. The kind is a small enum
  naming the problem; it exists for the once-per-launch rule and so the record does not assume a single type (M6). The
  supervisor writes the text, because it holds the specifics the action depends on (which flag blocked injection, which
  vendor), and the UI renders it as peer text. On the wire the record is only sequence number, time and text: kind and
  launch stay database columns, because an enum on `SessionInfo` that a newer supervisor extends would make the whole
  record fail to decode, and the helm drops a session whose cached record does not decode. No registry, no subscription
  mechanism, no severity levels.
- P2. Supervisor storage: a table in the supervisor's database (`crates/farhelm-supervisor/src/store.rs`, with its
  schema-migration conventions), keyed by session, capped at 10 per session on insert, deleted with the session. The
  once-per-launch rule is a unique key there (session, generation, kind), so it survives a supervisor restart; no
  separate latch table is needed (a current-launch record can only age out behind 10 newer ones, and one launch makes at
  most one per kind). The existing tripwire's in-memory latch (`hook_warned`) stays; only its clock's start changes
  (M10). `report_liveness_tripwire` is a synchronous function under a std mutex and `with_hook_argv_using` has no store
  access by design, so recording happens in their async callers (`capture_now`, the spawn path that consumes `hooked`),
  not inside them.
- P3. Transport: the session's notifications (at most 10 short records) ride on the session's existing listing record
  (`SessionInfo` in `crates/farhelm-proto/src/lib.rs`, flattened into the helm's `SessionRow`), and recording one sends
  the `SessionsChanged` hint. Add the field with a decode default. The version rules in
  `crates/farhelm-proto/src/lib.rs` allow a new optional field when ignoring it is harmless, which holds here (an older
  helm shows no bell; an older supervisor sends none), so expect no protocol bump; bump only if you find a rule that
  requires it, and log why. A bump makes mismatched helm and supervisor builds refuse to connect.
- P4. Read and cleared state live in the helm database next to the existing per-session `session_seen` table
  (`crates/farhelm-helm/src/store.rs`), as two per-session sequence marks: read through N and cleared through N. The
  helm leaves out cleared notifications and computes which are unread when it builds rows for the UI. Two small helm API
  call (or two) sets the marks, modelled on the seen-stamp endpoint, on popover close and on clear; like `mark_seen` it
  bumps the fleet-events revision so other clients of the helm update without waiting for a poll. Shipping the marks on
  the row and filtering in the UI, the way `seen_activity_at` works, is equally acceptable. Clearing does not delete
  anything in the supervisor; the cap ages records out there. This needs no new supervisor request.
- P5. UI: a bell glyph in `crates/farhelm-ui/src/icons.rs` (follow `docs/harness-marks.md`'s sizing rule and the
  module's conventions), and the bell control and popover in `crates/farhelm-ui/src/list/row.rs`. Position the popover
  with the shared floating-panel code in `crates/farhelm-ui/src/menu_panel.rs`, which the `⋯` menus already use and
  whose docs record that a second hand-rolled copy was tried and rejected. The list is not a menu of actions, so the
  menu's roving-focus machinery (`MenuOrder`, `MenuWiring`) need not apply. The row's open control is itself a button,
  so the bell must be a sibling control laid over the row the way the `⋯` menu button is, not nested inside it.
- P6. Wording per kind follows M5. Where a problem was caused by a deliberate configuration (for example the user turned
  hooks off for that agent through `FARHELM_AGENT_HOOKS`, if that still exists after the launch redesign), proposal: do
  not notify, since the user chose it; log a DECISION either way.
- P7. Tests: supervisor tests for the cap, the once-per-launch rule across a store reopen, and each kind's recording
  site; helm tests for read and cleared marks and row building; UI render tests for bell states; a browser spec for the
  bell's appearance, the unread style, the popover, read-on-close, and clear. The Playwright stack runs a plain
  `cargo build` binary without the supervisor's `test-seams` feature, so do not build a seams-enabled binary for it: the
  browser spec stubs the session listing with `page.route`, as several existing specs do, and the Rust e2e suite (which
  has the seams) covers recording reaching the helm, without waiting 65 real seconds.

Agreed fallback: if a listed tracking problem has no action the user can take and no meaningful consequence to state
(M5), drop it from this version with a DECISION and list it in the report as a possible follow-up, rather than shipping
a notification that says nothing useful.

## Implementation outline

Moderate, across all three components and the protocol, but each piece is small: one supervisor table and a recording
call at each listed site's caller; a protocol field (no bump expected); two helm marks, their store migration and two
API calls; a bell, a popover and their CSS in the UI; spec and docs. No new subsystem, no new event channel (the
existing listing and hint carry it), no OS or desktop notifications, no sound.

Suggested stack, bottom up (shape it differently if review would be helped, without churn):

1. `docs`: SPEC.md and SPEC_impl.md changes (M1): remove the two out-of-v1 statements, describe session notifications
   and their storage, rule and transport.
2. `feat`: the tripwire clock change (M10), supervisor recording and storage, the protocol field, every listed recording
   site, tests. The clock change may be its own `fix` PR below this one if that reads better.
3. `feat`: helm read and cleared marks, row building, API calls, tests.
4. `feat`: the bell and popover in the UI, browser spec, user docs, changelog fragment, removal of the TODO.md entry.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-session-notifications-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/session-notifications/<nn>-<short-name>`.
- Shape the stack per the outline. Err on the side of bite-sized PRs, without churn: no code added in one PR and deleted
  in a later one. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits; `feat` PRs carry a changelog fragment under `releasing/changelog.d/` in the same commit, per
  root `AGENTS.md` (Releases and the changelog), with `kind: none` and a reason on the ones that are not user-visible on
  their own; validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).
- The last PR removes the TODO.md entry "Notification system".

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, nextest selections of
the supervisor store and capture modules, the helm store and aggregation, the protocol crate and `farhelm-ui`,
`python -B scripts/check-test-sleeps.py`, `dprint check` on changed files, the website build when docs change, and the
new browser spec plus the sidebar specs on Chromium and WebKit through the recorder. Say in the report which checks ran
and why.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to TWO
fresh-context reviewers, as the user demands (M8): a Claude Opus 5.5 agent at high effort, and a gpt-6-astra agent at
high effort (shelled out to the other harness if the executing one cannot reach that model natively). No review swarm.
Both get the same prompt, which carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory, one per reviewer), and the acceptance criteria: The goal, Decisions already made, and
the outline above. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim. Address what both reviewers find before moving on, and log the DECISION where you decline a finding. Do not
write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a separate event channel or stream for
notifications, a notification registry or subscription mechanism, host-level or app-wide notifications, OS or desktop
notifications, a supervisor request to delete notifications, severity levels; these are examples, not a blacklist), and
whenever the same component has needed repeated corrective review rounds, run a fresh-context review through
galaxy-brain with this charter, supplying the request, the decisions above, this outline, the current diff and the
proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the record shape and who writes the text (P1), the final list of recording sites
after the launch redesign and any site dropped under the agreed fallback, how "launch" is identified for the
once-per-launch rule, the deliberate-configuration rule (P6), the evidence that terminal replies carry no carriage
return (M10), the classification of every refusal site, the exact text of every notification and the action it names,
the unread style, whether the protocol needed a bump, and every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. The one agreed fallback is dropping a tracking problem that has nothing actionable to say,
under Decisions already made. Anything else that needs a decision (for example, the launch redesign having changed the
sidebar row so that the bell's agreed place no longer exists): record the concrete tradeoff and block per
`plans/AGENTS.md` (Executing one plan, step 10).

## Done criterion

The plan is complete when its linear stack of draft PRs exists, satisfies the acceptance criteria, has passed the review
gate, and has removed the TODO.md entry. Open, not merged: merging happens only after the maintainer has reviewed this
plan's report. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing
entry in its log, and stop the watchdog. Never edit `plans/` yourself.
