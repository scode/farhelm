# The supervisor reads agent transcripts only when a decision needs them

Written against main at 46f78dcb on 2026-10-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan runs after `conversation-notice-hook-restart.md`: that plan changes the supervisor's notification and capture
pass code this plan also touches, so start only once it has left `plans/queue/INDEX.md`.

## The goal

On an ordinary host with about ten live sessions, the supervisor spent a large share of a core polling state that had
not changed (`lore/2026-10-08-supervisor-idle-cpu.md` has the measurements). The largest single item was re-reading
every hook-reported Codex session's transcript header on every capture pass: 64 KiB read, copied, and the 23 KB first
line parsed, about once a second per session, with Grok's record pair re-checked the same way. Around it sat smaller
per-pass costs: three failed report-slot renames per session folder even when the folder is empty, a second read of the
same session row in the Codex and Grok refresh, SQLite statements parsed afresh on every hot read, and a
notification-resolve write transaction opened on every pass whether or not anything could be resolved.

Make the supervisor read a transcript only when a decision functionally depends on content no hook provides, and remove
that surrounding overhead.

Acceptance criteria:

- No Codex transcript or Grok record pair is read by the periodic capture pass, by a session-list or session-info
  request, or by a replayed session create, for any session in any state (D2, D3).
- The reads that remain are the ones with a functional reason: report admission (whether the report is about the root
  conversation or a subagent thread, the runtime id match, and the persistent thread id Resume needs), a later Codex
  hook report that confirms a pending `/clear` (D1), Grok's enrichment admission, and Restart's click-time check. Every
  one of them that reads a transcript header stops at the first newline, under the existing 64 KiB ceiling, and makes no
  second copy of what it read (D4). Grok's `summary.json`, a complete document, keeps its complete-file reader.
- After a Codex `/clear` whose transcript did not exist when its `SessionStart` report arrived, the session becomes
  resumable when a later subscribed Codex hook report arrives for that same conversation, with no background polling
  (D1). That holds when the supervisor was down while the user cleared and took a turn: both reports survive on disk and
  are applied in order when it returns. A later-event report for a session with no pending clear causes no transcript
  read.
- A Codex or Grok transcript deleted after admission is discovered when the user chooses Restart: Restart refuses
  without launching anything, the resume offer is withdrawn, and the existing "resume withdrawn" notification is
  recorded then (D2). A file found holding a different conversation withdraws the offer without a notification, as today
  (D7). The session list may say Resume until that click.
- A file that cannot be read at Restart for a passing reason (an I/O error, not a missing or mismatched file) refuses
  that one Restart with a message saying to try again, and leaves the resume offer in place (D6).
- SPEC.md states the general principle in D2, and SPEC.md, SPEC_impl.md and the docs website no longer say a capture
  pass or reconciliation re-checks these files. SPEC.md's notification wording ("missing or inconsistent") matches D7.
- The report-folder drain skips the slot renames when the folder listing shows no report file; when any report file is
  present it keeps the full take order that Grok's selection and enrichment pairing depends on.
- The duplicate session-row read in the Codex and Grok refresh is gone, if that refresh still runs anywhere it is
  redundant after the work above.
- The store's hot reads use cached prepared statements.
- The notification-resolve write transaction runs only when a notification it could resolve exists, with SPEC_impl.md
  edited where it currently accepts the unconditional call.
- The last code PR removes both TODO.md entries this plan covers: "Read agent transcripts only when functionally
  needed." and "Cut the supervisor's remaining per-sweep overhead."
- A linear stack of draft PRs, each having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, the maintainer's words), two Near term entries merged into this plan:**

"Read agent transcripts only when functionally needed. The supervisor re-reads every hook-reported Codex session's
transcript header on every sweep (about once a second each: 64 KiB read, copied, and the 23 KB first line parsed), and
Grok has an equivalent per-sweep re-check. With hooks as the source of conversation identity, a transcript is read only
when a decision functionally depends on content no hook provides: not as a defensive fallback, and not because SPEC.md
or SPEC_impl.md currently asks for it (where they do, the specs change). Candidate needs to settle when planning:
telling a root Codex conversation from a subagent thread at admission (the hook payload does not say; the header's
`source` does), and noticing the new transcript after a `/clear` whose hook fired before the file existed (until it
exists, or via a later hook event). Decide whether Restart needs a check at all, and if so do it there rather than on
the sweep. Any read that remains stops at the first line. Findings, data, and the options weighed:
`lore/2026-10-08-supervisor-idle-cpu.md`."

"Cut the supervisor's remaining per-sweep overhead. Skip the three report-slot renames when a session's report folder
listing shows no report file (today every folder costs three failed renames and three UUIDs per sweep, stopped sessions
included); when any report is present, keep the full take order Grok's selection and enrichment pairing depends on. Drop
the second read of the same session row in the Codex and Grok refresh. Use cached prepared statements for the hot store
reads (the crate has none; statement parsing dominates the session-row read). Run the notification-resolve write
transaction only when a notification it could resolve exists (SPEC_impl.md currently accepts the unconditional call, so
that part is a spec edit). Details: `lore/2026-10-08-supervisor-idle-cpu.md`."

**The user's decisions (2026-10-08):**

- D1. A pending Codex `/clear` is confirmed by subscribing to a later Codex hook event (`Stop` or `UserPromptSubmit`,
  whichever fires after Codex has written the new transcript and names it), and recording the conversation when that
  report arrives. The maintainer's question that settled it: "Why is the default behavior simply to wait for the hook
  that will fire after the first turn in /clear and then record it?" No background polling, and no new restart-offer
  state. If neither event works on current Codex (it does not fire after the file exists, or its payload does not name
  the transcript), stop and block per Unattended fallback below; do not fall back to polling or to a Restart-time-only
  confirmation.
- D2. A transcript deleted or replaced after admission is noticed only when the user clicks Restart. The maintainer's
  words, which also set a general principle for SPEC.md: "if the spec needs a change here - we do not add a bunch of
  complexity to account for 'someone might have deleted arbitrarily files at an arbitrary time on the host that is
  considered trusted' unless the consequences of not doing so are _sever_". Restart's existing click-time check is what
  keeps a stale offer from launching the wrong thing.
- D3. The two TODO entries are one plan, because they change the same capture-pass code.
- D4. Remaining header reads stop at the first line (the request's own words).
- D5. Review gate: a fresh-context gpt-6.1-sol agent at high effort with the general charter, no swarm.
- D6. A passing read error at Restart refuses that Restart and keeps the resume offer, so the user can retry; it never
  withdraws the offer for good. (Today the background check would have restored a withdrawal like that on a later pass;
  with the background check gone, nothing would.)
- D7. A file at the reported path that now holds a different conversation keeps today's behavior: the offer is withdrawn
  and no notification is recorded; only a missing file notifies. The maintainer accepted this on the premise that a
  mismatch is an unexpected, something-is-buggy event and not what an ordinary `/clear` produces. The planner's reading
  is that a Codex `/clear` and a Grok `/new` write the new conversation to a new file at a new path, which the report
  names, so the old path is never rewritten. Confirm that premise while doing PR 2; if an ordinary flow can produce a
  mismatch at a reported path, block with that finding.
- D8. The upgrade gap for Codex sessions launched before this change (a `/clear` in one of them is not confirmed until
  relaunch) is called out as a breaking change in the PR that adds the later Codex hook event: a `!` on its title type,
  a `kind: breaking` changelog fragment, and a statement in its PR description. The maintainer asked for exactly this.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**What the planner found (at 46f78dcb).**

- The per-pass work starts in `crates/farhelm-supervisor/src/service/capture.rs` (`refresh_report_only_captures`), which
  reads each integrated session's row and calls `refresh_reported_capture_claimed` in `service/core.rs`. For Codex and
  Grok that reloads the row again and calls `refresh_codex_capture_claimed` or `refresh_grok_capture_claimed` in
  `service/core/vendor/{codex,grok}.rs`, which verify the exact file and, on a withdrawal, record the `ResumeWithdrawn`
  notification.
- Restart already checks at click time for both agents. `restart` in `service/core.rs` reads `session_snapshot`, which
  calls `refresh_reported_capture` (taking the capture claim) and so runs the same Codex and Grok verification; Grok
  then checks again in `verify_grok_resume`. A failed Codex check leaves the offer `NotCaptured` with a captured
  conversation, and `unverified_resume_refusal` refuses the restart; Grok is refused through the offer's
  `unavailable_reason`. `restart` is `session_snapshot`'s only production caller. `replay_created_session` (the
  idempotent replay of a session create) calls `refresh_reported_capture` directly, so a replay also reads transcripts
  today.
- Today's verdicts: a missing file is a clean verdict and notifies; a mismatched file and a read error are both treated
  as inconclusive (Codex's `verify` returns an error, Grok's returns `false`), withdraw Resume, and do not notify. D6
  changes the read-error case at Restart; D7 keeps the mismatch case.
- Codex admission (`report_codex_conversation` in `service/core/vendor/codex.rs`) verifies the exact file through
  `CodexLocator::verify` in `agent_kind/codex.rs`. A `clear` report whose file is not yet a valid root record is
  admitted as a non-resumable pending locator; today only the capture pass later promotes it.
- Farhelm injects exactly one Codex hook event, `SessionStart` (`hook_argv` in `agent_kind/mod.rs`, a
  `-c hooks.SessionStart=...` value). Codex fires it at the first prompt of a fresh launch, and at the `/clear` itself
  for a cleared conversation. Whether `Stop` or `UserPromptSubmit` fires after the new transcript is written, and
  whether their payloads carry `transcript_path`, is unverified; check it against current Codex before building on it.
  The repository pins facts about real vendor hook behavior in `real_codex_session_reports_its_identity_across_new`
  (`real_agent_capture.rs`); pin this one there too.
- Three places refuse a Codex report that is not a `SessionStart` with a foreground `source`, and a `Stop` or
  `UserPromptSubmit` carries no `source`: `foreground_source_refusal` at the report doorway, `is_foreground_source` in
  `report_codex_conversation`, and `CodexLocator::reported`.
- A session's reports land in per-slot files. `HookReport::slot` in `crates/farhelm-supervisor/src/hook_report.rs` puts
  every non-Grok report in the single `latest` slot, and Grok's `SessionStart` in `selection` and its other events in
  `enrichment`; the drain takes and applies selection before enrichment. With one Codex slot, a later-turn report would
  overwrite a `SessionStart(clear)` the supervisor has not applied yet (most plainly while the supervisor is down, which
  SPEC.md promises still applies on return), and a `UserPromptSubmit` would race a fresh launch's own `SessionStart`,
  since both fire at the first prompt.
- `hook_command` adds the agent-instructions pointer (`--announce`) to the hook it builds. A second Codex hook entry
  built the same way would print that pointer on every turn.
- Grok's `UserPromptSubmit` and `Stop` callbacks already re-verify its record pair at admission (`incoming.verify()` in
  `service/core/vendor/grok.rs`), so a pending Grok selection already resolves through hooks; SPEC.md says "A fresh
  `/new` normally remains pending until a later subscribed event supplies its path."
- The shared bounded reader is `read_prefix` in `agent_kind/records.rs` (re-exported as `read_bounded_regular_file` in
  `agent_kind/mod.rs`). It reads the whole 64 KiB ceiling through tokio's file API and then copies it with
  `String::from_utf8_lossy`. Its header callers are Codex's `verify`, Grok's `updates.jsonl` check, and Pi and OMP
  through `read_record`. Two other callers compare complete multi-line files byte for byte and must keep reading the
  whole file: the OMP reporter-asset check in `service/core/vendor/omp.rs` and the extension check in `pi_extension.rs`.
  `read_complete` reads Grok's `summary.json`.
- The report-folder drain is `drain_session_dir` in `service/report_files.rs`. The notification resolve is
  `resolve_resumable_notifications` in `service/notifications.rs`, called from `service/capture.rs`. The store is
  `crates/farhelm-supervisor/src/store.rs` on rusqlite, with no `prepare_cached` anywhere.
- Spec text to revisit: SPEC.md's Status notification paragraph ("a Codex or Grok conversation record that Farhelm
  re-checks and finds missing or inconsistent"), the Codex and Grok paragraphs of the conversation-identity section
  ("Farhelm checks that pair during reconciliation and again before Resume"), SPEC_impl.md's agent-kind integration
  bullet ("Reply paths and the ticker both run this reconciliation"), its Codex and Grok exact-record paragraphs
  ("Capture passes and resume verification re-read that exact file", "Reconciliation rechecks that pair"), and its
  notification paragraph ("the background refresh and Grok's final check before a Restart both record it", "No per-entry
  latch suppresses these calls"). The docs website's `website/src/content/docs/docs/agents/codex.md` and the Grok and
  hook-injection pages may describe the same; check them.
- The e2e Codex tests (`crates/farhelm/tests/e2e/codex_identity.rs`) drive a fake Codex
  (`crates/farhelm-fixtures/src/fake_agent/codex_conversation.rs`) that will need to emit the new hook event.
- `conversation-notice-hook-restart.md` was complete but not landed when this was written, and it edits `capture.rs` and
  `notifications.rs`. Re-check these findings against main once it has landed.

**Binding repository constraints:** root `AGENTS.md`: Harness-specific code (read the map in
`crates/farhelm-supervisor/src/agent_kind/mod.rs` first; no `kind == X` or harness-specific `_` arms in shared code),
Finishing work (targeted validation, the test-sleep check, the recorder), Releases and the changelog (fragments for
`perf`, `fix`, `feat`), Testability (no tests that change the process environment), Sharing the machine, Agent scratch
space, The live install is off-limits, and Docs website (read `website/AGENTS.md` and `website/EDITORIAL_RULES.md`
before editing a docs page). SPEC.md's "Upgrade compatibility and client scale". `.agents/test-authoring.md` for any
test change.

**Planner proposals** are the mechanisms in Outline that the decisions do not fix, including the PR slicing, the
preference for `Stop`, and the treatment of sessions launched before the upgrade. Challenge them through Scope
reassessment rather than treating them as requirements.

## Outline

The PR slicing below is a proposal; reshape it if the code argues for it, without churn.

### PR 1: header reads stop at the first line

`perf:`, with a changelog fragment (`kind: none` is fine if nothing is user-visible). Add a first-line reader beside
`read_prefix` that stops at the first newline, keeps the 64 KiB ceiling as a ceiling, and makes no second lossy copy,
and move the header callers to it (Codex's `verify`, Grok's `updates.jsonl` check, `read_record` for Pi and OMP). Leave
`read_prefix` itself, or whatever whole-file reader you rename it to, for the OMP reporter-asset and Pi extension
checks, which compare complete files. Check each moved caller still gets what its parser needs; Codex's parser already
requires a complete first line. Unit tests for a first line shorter than, equal to and longer than the ceiling.

### PR 2: a later Codex hook event confirms a pending `/clear` (D1)

First establish, against the current Codex release, which of `Stop` and `UserPromptSubmit` fires after a post-`/clear`
transcript is written and whether its payload names `transcript_path`; record what you ran and saw in the log, since it
becomes the spec's evidence. Prefer `Stop` if both work (the file is certain to exist at the end of a turn). If neither
works, block (Unattended fallback).

Pin that finding in `real_codex_session_reports_its_identity_across_new` (or a sibling in `real_agent_capture.rs`).

Then:

- Add that event to the Codex hook injection, without the agent-instructions pointer: only `SessionStart` announces.
- Give Codex the same two-slot shape Grok has: `SessionStart` in the selection slot, the new event in the enrichment
  slot, drained selection first. `HookReport::slot` hard-codes Grok today; make the mapping a per-harness answer in the
  places the `agent_kind/mod.rs` map names, not a second `vendor == X` branch.
- Let the new event past the three refusal points above, for that event only. In admission it only confirms the current
  pending locator: same attribution corridor, same runtime session id, never selecting or replacing a conversation. It
  may fill an absent path only for that same runtime id. Promote the pending locator when the exact file now verifies. A
  later-event report for a session with no pending clear is dropped before any file read.
- Update the fake Codex, the e2e test for the pending clear (including a case where both reports wait on disk and are
  applied together), SPEC.md and SPEC_impl.md's Codex paragraphs, and the docs pages that list the injected hook. Add a
  changelog fragment.
- Codex sessions launched before the upgrade keep only the `SessionStart` hook, so a `/clear` in one of them is not
  confirmed until the session is relaunched. Do not add machinery for it (the D2 principle). The maintainer requires
  that this PR call it out as a breaking change (D8): the title's type carries `!` (for example `feat!:`), the changelog
  fragment is `kind: breaking` and says in user terms that Codex sessions started before the upgrade cannot resume a
  conversation begun with `/clear` until they are relaunched, and the PR description states the same.

### PR 3: no transcript reads on the capture pass (D2)

Remove the Codex and Grok exact-file verification from the capture pass, from any list or info path, and from
`replay_created_session`, which answers from the stored row instead. Keep Restart's click-time check, which already
records `ResumeWithdrawn` for both agents on a missing file; confirm that by test. At Restart, a read error refuses that
Restart with a message saying to try again and does not persist a withdrawal (D6); a missing file withdraws and
notifies, and a mismatch withdraws without notifying (D7). Write the D2 principle into SPEC.md (where the
conversation-identity section introduces exact-file checks reads best) and edit the spec and docs text listed above.
Behavioral regression tests: delete a resumable Codex session's transcript; the next passes leave the offer at Resume;
Restart then refuses, launches nothing, withdraws the offer and records the notification. A read error at Restart
refuses and leaves the offer at Resume. The same for Grok if its existing tests make that cheap. `perf:` with a fragment
that tells users a deleted transcript is now noticed when they choose Restart.

### PR 4 onwards: the remaining per-pass overhead

In whatever slicing reads best, as `perf:` PRs with fragments where anything is user-visible:

- The report-folder drain skips the renames when the listing shows no report file, keeping the full take order whenever
  any is present. Its existing tests (`a_pass_takes_every_slot_before_applying_any`, the Grok
  selection-before-enrichment test) must still pass.
- The duplicate row read in the Codex and Grok refresh, if PR 3 leaves that refresh running anywhere it is redundant.
  The reload under the capture claim in `session_snapshot`'s path is not a duplicate: it exists because the snapshot
  reads without the claim, and must stay.
- `prepare_cached` for the store's hot reads (the session-row read first; the lore file names the rest).
- `resolve_resumable_notifications` opens its write transaction only when a notification it could resolve exists, with
  the SPEC_impl.md notification paragraph edited to match. Its correctness rule (the resolve compares the exact captured
  conversation and generation) must survive. The session's in-memory notification list carries no kind or generation, so
  deciding "could resolve" needs either those fields in memory or a store read first; only the first actually removes
  the per-pass database call, so prefer it unless it turns out disproportionate, and log the DECISION.

The last PR removes both TODO.md entries.

### Out of scope

The "Sweep on the timer only" TODO entry is separate: do not change what triggers a capture pass or how often. Do not
touch screen captures, SQLite's journal mode, or the allocator (all named in the lore file as not pursued or as later
work).

### Validation

Per root `AGENTS.md` Finishing work, choose targeted checks per PR rather than a battery: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`; the affected supervisor
unit tests and the Codex and Grok e2e tests through `scripts/record-test-run.py` with nextest; the test-sleep check when
tests change; `cd website && bun install --frozen-lockfile && bun run build` when a docs page changes; `dprint check`
and `python3 releasing/check-changelog.py format`. A real Codex run is needed only for PR 2's event check; it spends a
vendor turn or two, which is acceptable, and must run in a session you start, never in the live install.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-transcript-reads-on-need-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
work.

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
  `plan/transcript-reads-on-need/<nn>-<short-name>`.
- Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack deletes. Within this run, if
  a PR needs correcting, restructure it rather than stacking a correction on top; that applies to all of this plan's own
  open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entries this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code, tests or scripts, use the active galaxy-brain skill to delegate a review of
that PR's changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review
swarm: a fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the
executing one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria for that PR: its part of Outline, the decisions that apply to it, and
the goal's acceptance criteria. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (a background poll for pending clears, a cache of
file metadata to decide when to re-read, a new restart-offer state, a compatibility path for sessions launched before
the upgrade; these are examples, not a blacklist), and whenever the same component has needed repeated corrective review
rounds, run a fresh-context review through galaxy-brain with this charter, supplying the user's request and decisions
above, this outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler
alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular which Codex event was chosen and the evidence for it, whether D7's premise held,
how a confirming report with an absent path is treated, how the slot mapping became per-harness, where the D2 principle
went in SPEC.md, how "could resolve" is decided for notifications, which store reads got cached statements, the final PR
slicing, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
D1's "neither event works" case is such a block. Because the PRs form one linear stack, later PRs sit on top of a
blocked one: finish the PRs before it, record the question, and close the plan as blocked rather than building past it.
PR 1 and the overhead PRs do not depend on D1; if D1 blocks, you may order them below PR 2 so they are built and
reviewed before the block.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code, tests or scripts passed the review gate. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.
