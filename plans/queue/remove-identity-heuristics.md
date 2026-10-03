# Remove Claude's record scan: conversation identity comes only from the agent's own report

Written against main at a7c477fd on 2026-10-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says. Line numbers below are from main at a7c477fd and will have moved; use them to find things, not as facts.

This plan has no dependency on another plan. One neighbor touches nearby ground: `identity-report-wait-retry.md` (in
flight when this was written) changes how the supervisor waits for the per-session capture claim during report
admission, Claude's sender check, and the hook's time budget, in `crates/farhelm-supervisor/src/service/core.rs`,
`crates/farhelm/src/hook.rs`, SPEC_impl.md and the website's hook page. If it has landed, read what it changed and build
on it; if its PRs are still open, keep clear of the claim-wait code and text it owns.

## The goal

For agents with a conversation-identity integration, the supervisor records which agent conversation belongs to each
session, so restart can resume exactly that conversation. SPEC.md "Durability and resume" now says identity comes only
from the harness's own explicit report (a hook, plugin, extension, or whatever reporting mechanism the harness needs),
never from heuristics, because a wrong match resumes and appends to a conversation that is not the session's own.
Checking the exact file a report names is verification of that report, not identification, and stays.

One heuristic is still in the code: Claude's record scan. When no hook report has been accepted for a Claude session,
the supervisor correlates the time of the session's first input with Claude's on-disk records under
`~/.claude/projects/<munged-cwd>/` and claims a conversation when exactly one record fits. It lives mainly in
`crates/farhelm-supervisor/src/agent_kind/capture.rs` and `crates/farhelm-supervisor/src/service/capture.rs`, with
pieces in the agent-kind trait, the store, the ticker, the connection's input path and several e2e suites. Codex used to
have the same scan and lost it earlier; Goose, Pi, OMP and Grok are report-only. A survey at planning time found no
other heuristic identity path for any kind (see Implementation outline), but the TODO entry asks for any other one found
along the way to go too.

After this plan:

- No code path derives a conversation identity from anything but an accepted report. Claude's scan, its capture windows,
  the first-input anchor it keys on, its ambiguity verdict, its durable claim and its re-verification of records it
  claimed earlier are gone.
- A Claude launch without an accepted report (a profile already passing `--settings`, a bare `--`, `FARHELM_AGENT_HOOKS`
  opting out, a wrapper chain deeper than one level, a hook that failed) has no captured identity and takes the
  uncaptured-identity fallback SPEC.md already defines: restart says so and offers a fresh launch.
- Sessions whose stored identity the scan claimed earlier keep that identity and keep offering Resume, exactly like a
  reported one (M3 below). There is one stored-identity state, not two.
- The "a hooked launch has had input for a while and none has reported" supervisor log warning still fires for every
  launch Farhelm injected a hook into (Claude and Codex included), on its own timer (M5).
- The database no longer has the scan-only columns (M4).

Acceptance criteria:

- Searching the supervisor for the scan's names (`scan_records`, `record_root`, `CaptureWindow`, `Provisional`,
  `PendingCommit`, `UncapturedFinal`, `Ambiguous`, `first_input_at`, `capture_ambiguous`, `captured_record`,
  `reverify_capture`, `munge_cwd`, `agent_home`) finds nothing, or only a migration rung and its test that name a
  dropped column.
- A Claude session launched with hooks disabled, given input and later restarted, offers a fresh launch, not Resume, and
  no capture state other than "no identity" ever appears for it. An e2e test shows this.
- A session whose row holds a conversation id with no `'hook'` source (a historical scan claim) offers Resume after a
  supervisor restart, and a Resume launches `claude --resume <that id>`. A test shows this.
- Every test that used the scan only to obtain an identity gets it from a hook report instead and still tests what it
  tested; tests of the scan itself are deleted. Classify each test by what it actually pins (P4); a test whose property
  only the scan exercised is a scan test, even if it looks like an identity consumer.
- The no-report warning has unit tests (it is a pure function of the sessions, its timeout and the current time; no e2e
  test, which would need the timeout as a seam) showing it fires once for a hooked launch that holds no identity, for
  both a Claude and a Codex session, and not once a report has arrived. A Resume relaunch that carries its identity over
  is not warned about even if its hook stays silent; that is accepted, because the carried identity is still the right
  resume target, and it needs no per-launch "reported this launch" flag.
- A schema migration drops the scan-only columns; the fresh schema and the migrated schema agree, and a database at the
  previous version migrates with its sessions, identities and offers intact.
- SPEC.md, SPEC_impl.md, the website's agent pages and the code comments no longer describe the scan as existing or
  pending removal; the remaining text describes reports, verification of reported files, and the fallback.
- Report-only kinds behave as before: their per-pass reconciliation and exact-file verification (Codex, Grok, Pi, OMP,
  Goose) still run on every path that ran them.
- Each PR has passed the review gate; the last PR removes the TODO.md entry. No PR is marked ready, nothing is merged.

## Requirement sources

**The user's request**, the TODO.md `Near term` entry "Remove heuristic conversation-identity fallbacks", verbatim:
"Decided 2026-10-01: Farhelm identifies an agent's conversation only from the harness's own explicit report (a hook,
plugin, extension, or whatever reporting mechanism that harness needs), never from heuristics that cannot be relied
upon. Remove Claude's record scan (`agent_kind/capture.rs`, `service/capture.rs`, and their e2e suites; roughly 6k
lines) and the re-verification of records it captured earlier, plus any other heuristic identity fallback found along
the way. Launches without a report (a profile already passing `--settings`, a bare `--`, `FARHELM_AGENT_HOOKS` opting
out, a hook that failed) take the fallback SPEC.md already defines for an uncaptured identity. Review feedback that is
only true because this code still exists is discarded rather than fixed. Alert the maintainer before landing if existing
scan-captured sessions would lose a valid Resume offer."

**The user's decisions (2026-10-03):**

- M1. No-workhorse mode (below).
- M2. Review gate: a fresh-context Opus 5.5 agent at high effort with the general review charter (below).
- M3. Scan-captured and reported identities are the same thing for Resume. Asked whether historical scan claims should
  keep offering Resume, the user asked why it was a question at all: Resume uses the one stored conversation id whatever
  its source, and Claude's offer check never looks at the source. Agreed: collapse the two in-memory states into one
  stored identity, delete the re-verification, and do not reject a row because of where its id came from. Keep the
  `conversation_source` column: other code uses it (launch evidence in `service/core.rs`). Since no session loses a
  Resume offer, the TODO entry's "alert the maintainer before landing" condition does not trigger; if execution finds
  any case where it would, that is a block (Unattended fallback).
- M4. Drop the scan-only database columns with a schema migration rather than leaving them inert. The precedent is the
  rung that dropped `archived` (`ALTER TABLE sessions DROP COLUMN archived`, 18 to 19, in
  `crates/farhelm-supervisor/src/store.rs`).
- M5. Keep the no-report warning (`report_liveness_tripwire` in `service/capture.rs`), on an in-memory timer and a
  constant of its own instead of the scan's durable first-input anchor and capture horizon. It is a supervisor log line,
  not a user-facing notice, and stays one: surfacing such warnings to users is a separate TODO entry ("Notification
  system"), not this plan. The user wants it to cover Codex as well as Claude; it already covers every launch Farhelm
  injected a hook into (`RunCells::hooked`: Claude and Codex, and also Goose, Pi and OMP), and that must stay true. Do
  not encode a Claude/Codex-only rule in it or its tests.

**Binding repository rules:** root `AGENTS.md` (Harness-specific code: per-harness behavior goes where the module docs
of `crates/farhelm-supervisor/src/agent_kind/mod.rs` say, never `kind == X` in shared code; Finishing work; Releases and
the changelog; TODO.md; `.agents/test-authoring.md` for test changes; `python -B scripts/check-test-sleeps.py` for Rust
test changes), `website/AGENTS.md` and `website/EDITORIAL_RULES.md` before editing the docs site, and `plans/AGENTS.md`.

**Planner proposals** are marked P below, each with its reason. Change one if the code says otherwise, and log a
DECISION.

## Implementation outline

Mostly deletion, around 6 to 7 thousand lines, with a few hundred lines of test rework and the migration. Paths are
relative to `crates/farhelm-supervisor/src/` unless shown otherwise.

**What goes (scan-only):**

- `agent_kind/capture.rs`: the window constants and types (`CAPTURE_WINDOW_*`, `CAPTURE_PUBLICATION_GRACE`,
  `CaptureWindow`, `CaptureWindowBounds`, `CaptureVerdict`), the walk and its budgets (`scan_records`, `ScanOutcome`,
  `Candidate`, `choose`, `MAX_*`), and their tests.
- `service/capture.rs`: the scan ladder (`Unclaimed`, `Provisional`, `PendingCommit`, `UncapturedFinal`, `Captured`,
  `Ambiguous`, with `rank`/`is_settled`), `FirstInput`, `note_first_input`, `persist_first_input`, `reported_ids`,
  `is_spoken_for`, the scan body of `capture_pass`, `commit_capture`, `declare_ambiguous`, `persist_ambiguity`,
  `overlapping_windows_reason`, `reverify_capture`, and their tests.
- `CaptureWrite::{FirstInput, Conversation, Ambiguity}`; the trait methods `record_root`, `record_depth` and
  `is_record_file` with their impls; Claude's `parse_record`, `leading_json_lines`, `RECORD_PREFIX_LINES` and
  `munge_cwd`; the store writers `record_first_input`, `record_captured_conversation` and `record_capture_ambiguous`;
  the `agent_home` and `capture_window` seams; the durable first-input write spawned from the input path
  (`persist_first_input`; the call site in `service/connection.rs` stays, in-memory only, for the warning);
  `RecordStamp` and `stamp_of` (drop the stamp from `read_record`'s return, which Pi and OMP verification keep); the
  reload mapping that turns a non-hook row into `Captured { record }`.
- The pass coordination (decided at planning, P2): `CaptureCoordination`, `CaptureReason`, `CaptureHistory`,
  `capture_passes_completed`, the `Box::pin` around the pass, the `capture_gate` seam (keep the `CaptureGate` type,
  which `codex_report_gate` uses), and the coalescing tests in `service/ticker.rs`. Its docs justify it entirely by scan
  cost and scan races (one pass's provisional claim overwriting another's ambiguity). The no-home branch of
  `capture_pass_for` already runs the remaining work (report-only reconciliation plus the warning) in production with no
  lock and no coalescing, and `refresh_report_only_captures` takes each session's own `capture_locks` claim, so its
  correctness does not depend on the pass lock. Without coalescing, refresh runs somewhat more often on a polled
  supervisor (a row read per integrated session and a bounded header read per Codex or Grok exact file), which is
  negligible at the scale SPEC.md targets.

**What stays, because report-only kinds or verification use it** (P1: verify each before deleting its neighbors):

- `refresh_report_only_captures` and every call site of the pass that runs it (ListSessions, session info, restart,
  reload, startup, the ticker). The pass becomes one plain function, the body of today's no-home branch of
  `capture_pass_for` (reconciliation plus the warning), called by all of them. The ticker's call stays: it is what
  evaluates the warning and mirrors report-only state on a supervisor nobody is polling.
- `CaptureState::Reported` and the report path (`advance_capture` and the committed-identity accessors), the capture
  claim `capture_locks`, `CaptureStoreFault` with `CaptureWrite::Report`, and the `CaptureGate` type that
  `codex_report_gate` uses.
- `AgentIntegration::parse_record`/`read_record` where Pi and OMP use them for exact-file Resume verification; Codex's
  own `parse_record` with `RecordCorrelators`, `correlators_from` and `parse_rfc3339`; `format_rfc3339` (fixtures use
  it); `RECORD_PREFIX_BYTES` and the hardened readers `read_prefix`, `read_complete` and `open_regular_file`, used by
  Codex, Grok, Pi and OMP. Move them out of `agent_kind/capture.rs` into a module whose name says what they are if the
  file would otherwise hold only them. After the scan goes, the trait's `parse_record` has real consumers only in Pi and
  OMP (Codex's verifier calls `codex::parse_record` directly), and `RecordCorrelators`' cwd and timestamp fields are
  read by nothing that remains; do not redesign that here, because `correlators_from`'s validation is still part of
  Codex's acceptance of its exact file. Do rewrite the trait docs so they stop describing scan semantics.
- `CaptureWrite` keeps only `Report`; collapsing it into a plain report-fault seam is optional.
- `canonical_cwd` (used by `ensure_cwd_identity`).

**The state after the change** (P2): one in-memory capture state with "no identity" and "identity" variants (whatever
the existing `Reported` carries, ownership version included), loaded from the row's `captured_conversation` whatever its
source. A historical scan claim reloads as an identity and offers Resume through the same `restart_offer` path as a
reported one. `refresh_report_only_captures` already skips rows whose source is not `'hook'`, so historical scan rows
are loaded once and left alone afterwards, which is right now that nothing re-verifies them.

**The warning** (M5, P3): a launch-scoped in-memory instant set at first confirmed input (where `note_first_input` is
called today, without the durable write), and a constant of its own replacing the capture horizon (about the old window
plus grace, 65 s, is fine). The anchor stays at first input, not launch: Codex reports at first prompt, so a timer
anchored at launch would warn about every idle Codex session. What M5 replaces is the durable `first_input_at` and the
window-derived horizon, not first input as the event. The condition is "hooked launch, no identity held". Keep its
once-per-launch latch and its tests; add the kind coverage the acceptance criteria name.

**Tests** (P4), under `crates/farhelm/tests/e2e/` unless shown otherwise:

- PR 1 shares only helpers that survive PR 2: the hook machinery the converted suites need (`ServeTask`, which
  `codex_identity.rs` and `restart_with_resume.rs` already import from `hook_identity.rs`, `hook_session` /
  `fixture_invocation`, `attach_ready`, `report`) and neutral helpers (`snapshot_of`, `marker_value`,
  `last_marker_value`). Scan helpers (`record_session`, `provoke_record`, the `agent_home`/`capture_window` wiring in
  `capture_harness_with_seams`, `test_capture_bounds`, `settle_past_horizon`, `wait_for_capture`,
  `wait_for_first_input`, `TEST_CAPTURE_*`) stay in `conversation_identity_capture.rs` and die with it in PR 2. One line
  of churn is unavoidable and expected: while the scan still runs in PR 1, any harness the converted tests use must keep
  pointing `agent_home` at a private directory, or the supervisor falls back to `$HOME` and scans the real
  `~/.claude/projects`; PR 2 deletes that line. `real_agent_capture.rs`'s HOOK section and `restart_with_resume.rs` set
  `agent_home` too and lose it in PR 2.
- Delete `conversation_identity_capture.rs`; the CAPTURE section of `real_agent_capture.rs`; in `hook_identity.rs` the
  scan-versus-report tests (`a_scan_cannot_override_a_report`, `a_report_before_the_scan_lands_is_not_clobbered`,
  `a_reported_id_is_excluded_from_a_rivals_candidates`, `a_report_clears_ambiguity`) and `record_timestamp`; in
  `restart_under_concurrency.rs` `a_fresh_relaunch_opens_a_new_capture_window_after_an_ambiguity`; in
  `session_rename.rs` `a_rename_before_first_input_still_captures_the_conversation` (it pins that a rename shares the
  first-input cell the input route pinned, which a hook report never exercises, so a converted version would pass
  against the bug it pins); the scan unit tests in `service/capture.rs`, `service/ticker.rs`
  (`capture_advances_on_the_ticker_with_nobody_polling`), `service/core.rs`
  (`reload_distinguishes_a_reported_identity_from_a_scanned_one`, the relaunch-reopens-window test), `store.rs` (the
  capture-column tests) and `agent_kind/mod.rs`.
- Switch to a reported identity, through the fake agent's `hook-report` script and `report <id>` command
  (`crates/farhelm-fixtures/src/fake_agent.rs`): `restart_with_resume.rs` (`captured_claude_conversation` and the tests
  using it, the late-capture test, `interrupted_session_resumes_its_conversation` and its wrappers; a hook variant
  already exists there), `wrapper_launch.rs` (the Resume and `{cwd}` tests; the `{cwd}` test used the scan's match on
  the record's cwd as a second oracle, which goes, while `assert_wrapper_got` keeps checking the substituted slot), and
  in `session_rename.rs` only the surviving half of `a_rename_reply_captures_and_offers_resume_without_a_list_first`
  (the rename reply reflects the current offer rather than a stale one; the "rename's own pass captured it" half is
  scan). The hook dials the supervisor socket, so these need a served supervisor, not only the in-process duplex
  harness.
- Remove the `capture_store_fault` variants, `capture_gate`, `capture_window` and `agent_home` seams that only scan or
  coalescing tests used; keep what report tests use.

**The migration** (M4): schema 23 to 24 drops `first_input_at`, `capture_ambiguous` and `captured_record` from
`sessions`, with the fresh DDL, `SESSION_COLUMNS` and positional reads, `insert_session`, `begin_relaunch`'s CASE
clauses and relaunch-basis comparison, the report writers that reset those columns, `StoredSession` and the
`SessionSnapshot` fields (`first_input_at`, `capture_ambiguous`) updated to match. `migrated_and_fresh_schemas_agree`
must pass, plus a migration test from 23 with a scan-claimed row, a reported row and an ambiguous row. Re-read the store
module docs on what each column meant before dropping it; if a column turns out to serve something besides the scan,
keep it and log the DECISION.

**Docs and comments:** SPEC.md "Durability and resume" (the "Claude's record scan ... pending removal" sentence, the
"until its removal, the record scan may still claim one" clause); SPEC_impl.md (the `AgentIntegration` paragraph, the
Codex-attribution paragraph's "falls back to the scan (pending removal)", the hook paragraph's "Claude's scan, pending
removal, still runs", and the ListSessions cost note about the capture sweep); the website's `agent-hook-injection.md`
and `agent-wrappers.md` pages under `website/src/content/docs/docs/agents/`; module and field docs in `service/mod.rs`,
`agent_kind/mod.rs`, `service/ticker.rs`, `procs.rs`, `store.rs`, `crates/farhelm-helm/src/manager.rs` (comments saying
ListSessions carries a whole-host scan), and the e2e suites' module docs. Rewrite the module docs of whatever remains of
the two capture modules to say what they do now. Leave `TRIAGE_OUTCOMES.md`, `lore/` and other plans' files alone.

**Other TODO and review items:** TODO.md's Code review entry "Capture columns after a failed non-Resume restart"
(`A6-C5`) is about `first_input_at` and `capture_ambiguous`; once those columns are gone, remove it in the migration PR
or narrow it to whatever part still applies, and say which in the report. Per the request, any `review_feedback_queue/`
item that is only true because the scan exists is not fixed by this plan; list them in the report so the maintainer can
discard them. Two that mention the scan or the capture columns, unclassified at planning time:
`claude-clear-report-dropped-on-claim-timeout.md` and `codex-draft-mistaken-for-question.md`.

**Other heuristics found at planning time:** none. Codex verifies the exact reported transcript, Grok derives a sibling
file from the reported path, Pi and OMP check the exact reported file before Resume, Goose reports through its own
`AGENT_SESSION_ID`, and Claude's `foreground_claude_emitter` attributes the reporter rather than choosing a
conversation. If you find one, removing it is in scope when it is clearly identification rather than verification; if it
is unclear, block.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-remove-identity-heuristics-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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
  `plan/remove-identity-heuristics/<nn>-<short-name>`.
- The stack, in this order (agreed with the user; split further only where a reviewer would genuinely be helped, and
  without churn, meaning no code added in one PR and deleted in a later one):
  1. Tests that rely on the scan only to obtain an identity switch to hook reports, and the shared e2e helpers move into
     the harness. Behavior unchanged; a `test:` PR.
  2. Remove the scan: the deletions above, the collapsed capture state, the warning on its own timer, the scan tests,
     and the SPEC.md, SPEC_impl.md, website and comment updates. This changes what users get (a Claude session whose
     hook never reported no longer gets a guessed Resume target), so it carries a changelog fragment; pick the
     Conventional Commit type by the user-visible effect per root `AGENTS.md`, and log the DECISION.
  3. The schema migration dropping the scan-only columns, the `A6-C5` TODO cleanup, and removal of the TODO.md entry
     "Remove heuristic conversation-identity fallbacks".
- Conventional Commits; changelog fragments under `releasing/changelog.d/` in the same commit where root `AGENTS.md`
  requires one, validated with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`). Within this run, a PR that needs correcting is restructured rather than corrected on top.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings` and
`cargo clippy -p farhelm --bins -- -D warnings` (the second matters here, because seams are being removed and the
shipped binary builds without `test-seams`); nextest selections of the supervisor's capture, store, core, ticker and
agent-kind modules and of the e2e suites this plan touches (`restart_with_resume`, `wrapper_launch`, `session_rename`,
`hook_identity`, `restart_under_concurrency`, `real_agent_capture`, plus any suite that imports the moved helpers);
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`; `dprint check` on changed Markdown; the website
build when its pages change. The deletion is wide, so consider a full workspace nextest run once at the tip of the
stack, and say in the report whether you ran it and why. No browser specs unless you identify a concrete browser risk.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to a fresh-context
Opus 5.5 agent at high effort, as the user demands (M2). No review swarm. The prompt carries the full charter, because
the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its findings file
(in the scratch directory), and the acceptance criteria: The goal, Requirement sources and the outline above. For a PR
that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim. For PR 2, ask the
reviewer explicitly whether anything a report-only kind relies on was deleted, and whether any path can still produce an
identity without an accepted report. Address what the reviewer finds before moving on, and log the DECISION where you
decline a finding. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a new identity source, a new persistent state or
column, a provenance-based offer rule, a per-launch "reported" flag, a user-facing surface for the warning, a rework of
the report-only reconciliation, keeping the pass coordination; these are examples, not a blacklist), and whenever the
same component has needed repeated corrective review rounds, run a fresh-context review through galaxy-brain with this
charter, supplying the request, the decisions above, this outline, the current diff and the proposed departure (what
changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular where the hardened readers end up, how each touched test was classified, the
warning's constant, the commit type of PR 2, any column kept rather than dropped, any other heuristic found, the `A6-C5`
outcome, and every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. Agreed fallbacks: a column that turns out to serve something besides the scan is kept
(logged); an unclear identification-versus-verification case blocks. Anything that would make an existing session lose a
Resume offer it has today blocks, per the TODO entry's "alert the maintainer before landing". Anything else that needs a
decision: record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).

## Done criterion

The plan is complete when its linear stack of draft PRs exists, satisfies the acceptance criteria, has passed the review
gate, and has removed the TODO.md entry. Open, not merged: merging happens only after the maintainer has reviewed this
plan's report. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing
entry in its log, and stop the watchdog. Never edit `plans/` yourself.
