# Resolve stale notifications: a session notification marks itself resolved when Restart can resume again

Written against main at 07aa85e5 on 2026-10-05. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file directs a change to them.

## The goal

A session notification (the bell on a sidebar row, SPEC.md "Status" and the notification lifecycle paragraphs after it)
is a record of something that happened. Two of its kinds describe a problem that can later go away on its own: a Codex
or Grok session whose resume offer was withdrawn because the agent's own record of the conversation went missing or
stopped matching ("Restart stopped offering to resume it"), and a session whose agent had not reported which
conversation it is in a minute after the first Enter. When the record comes back, or a late report arrives, Restart can
resume again, but the notification stays as it was: red and unread if the user has not opened it, warning about a
problem that no longer exists. After this plan, such a notification visibly marks itself resolved.

The TODO.md entry this plans, verbatim:

> **Clear a session's notification once its problem goes away.** A session notification (the bell on the sidebar row) is
> a record of something that happened, not a live state, so it stays until the user clears it even after the problem it
> describes has resolved itself. Two cases where that happens today: a Codex or Grok session whose resume offer was
> withdrawn because the agent's conversation record went missing or stopped matching gets a notification saying Restart
> can no longer resume, and when the record comes back and Restart can resume again, the notification still says it
> cannot; and a session told that its agent never reported which conversation it is in keeps that notification after a
> late report arrives and Restart works. The bell then shows a stale warning, red and unread if the user has not opened
> it yet, about a problem that no longer exists. Make a notification go away, or visibly mark itself resolved, when its
> condition stops holding. Deciding which of the two, and whether "resolved" counts as read, is part of the work.
> Deliberately left out of the session-notifications plan (its report lists it as a possible follow-up).

Acceptance criteria:

- When Restart can resume a launch's conversation (its restart offer is Resume), that launch's "never reported which
  conversation" and "Restart can no longer resume" notifications are marked resolved. This includes a report or a
  restored record that arrives across a supervisor restart, and notifications recorded before this change.
- A resolved notification stays in the list, visibly marked resolved (for example greyed, with a "resolved" marker), and
  counts as read: it never makes the bell loud, and the list does not set it apart as new.
- If the problem comes back in the same launch after being resolved, the same notification comes back: it loses its
  resolved mark and moves to the top as new and unread, so the bell is loud again, and it reappears if the user had
  cleared it. There is still at most one entry per kind per launch, however often it flips (Decision 3). In practice
  only the resume-withdrawn kind can come back: once a conversation is captured, the "never reported" kind cannot recur.
- "Hook could not be added" and the OMP reporter mismatch have no resolving event and are unchanged. Notifications of
  earlier launches of the session are left as they are.
- A resolution never marks a notification resolved while its problem holds: a resolve decided from an earlier reading
  must not undo a withdrawal that happened after it (outline, "The resolve guard").
- Mixed versions degrade to today's behavior: an older helm or UI shows resolved notifications as unresolved; a newer
  helm with an older supervisor shows nothing resolved.
- SPEC.md's notification paragraphs (the four sources and "reported at most once per launch" in "Status", and the
  lifecycle paragraph after it) describe resolution, read-ness and recurrence. SPEC_impl.md's notification storage note
  (its "`(session, generation, kind)` is unique, which is the whole once-per-launch rule") is updated, and so are the
  code docs that state the old rule: the module docs of `crates/farhelm-supervisor/src/service/notifications.rs` ("at
  most one per kind per launch", "Notifications are history", the past-tense rationale on `resume_withdrawn_text`) and
  the `newer_or_equal` doc. The docs page `website/src/content/docs/docs/using/session-list.mdx` ("When a row shows a
  bell") says resolved notifications are marked, following `website/AGENTS.md` and `website/EDITORIAL_RULES.md`.
- A changelog fragment under `releasing/changelog.d/` (kind `changed`) in the same commit as the behavior change.
- The last PR removes the TODO.md entry "Clear a session's notification once its problem goes away".

## Decisions already made

Settled with the maintainer during planning. Do not reverse them.

1. **Mark resolved, do not remove.** The notification stays, visibly marked resolved.
2. **Resolved counts as read.** It never makes the bell loud.
3. **A problem that comes back re-opens the same notification** at the top, new and unread, rather than adding a second
   entry or staying silently resolved. One entry per kind per launch.
4. **Only the two kinds named in the TODO resolve.**
5. **No downgrade path, accepted.** Moving the supervisor's store to the next schema version means an older supervisor
   refuses the database and will not start, including the one a downgraded desktop app manages. The maintainer was told
   and accepted it (SPEC.md, "Upgrade compatibility and client scale", requires surfacing this). The upgrade path must
   stay intact: an additive step on the forward migration ladder, no protocol change.
6. **Review gate:** two fresh-context reviewers per PR, Claude Opus 5.5 at high effort and gpt-6-astra at high effort,
   no review swarm (How to run, Review gate).
7. **No-workhorse mode** (How to run).

## Implementation outline

Planner proposals unless marked as a decision, grounded in main at 07aa85e5 and checked by a fresh-context planning
review. Line numbers are approximate. The prior plan's report, which lists this as a follow-up, is readable with
`git show b44076ba^:plans/reports/session-notifications.report.md`.

**Store** (`crates/farhelm-supervisor/src/store.rs`, `SCHEMA_VERSION` 27 today; take the next free version if another
change got there first). Add a nullable `resolved_at` column to `session_notifications` (the fresh schema ~1570 and a
ladder step like the 26→27 one ~2194). Keep `UNIQUE (session_id, generation, kind)`: Decision 3 keeps one entry per kind
per launch. Nothing reads the timestamp; do not grow it into a wire timestamp or a "resolved N ago" display.

- Re-open (Decision 3): when `record_session_notification` (~4884) meets an existing row for the key that is resolved,
  update it in place: new `seq` = current MAX+1, new `at`, the new text, `resolved_at` NULL, and return true so the cell
  reloads and the hint fires. Keep the existing generation and `unless_captured` guards. Do not run the cap-trim delete
  (no row is added), and do not implement it as delete-plus-insert. An existing unresolved row stays ignored, as today.
- Resolve: a new store call, roughly
  `resolve_session_notifications_if_current(session, generation, conversation, kinds)`, that sets `resolved_at` on
  unresolved rows of those kinds for that launch.

**The resolve guard.** Grok's check before a Restart (`verify_grok_resume`, `service/core/vendor/grok.rs`) can withdraw
the offer and re-open the notification without holding the capture claim. A resolve decided from an earlier "Resume"
reading that lands after that would mark the re-opened notification resolved, and nothing would re-open it again. So the
resolve's SQL requires the session row to still hold the exact captured conversation that justified it
(`... AND EXISTS (SELECT 1 FROM sessions WHERE id = ? AND generation = ? AND captured_conversation = ?)`), the same
compare-on-row discipline as `replace_reported_conversation_if_current`. Every withdrawal rewrites that string, so a
stale resolve is a no-op. Comparing an opaque string keeps this harness-neutral.

**Where resolution happens.** Not at a transition hook. `advance_capture` (`service/capture.rs`) is sync, runs under a
std mutex with `&Supervisor`, and misses a report admitted during supervisor startup before the session's entry exists
(`finish_reported_admission` returns early when `entry` is `None`) as well as sessions seeded already resumable.
Instead, add one reconciliation step at the end of `Supervisor::capture_pass`, after `refresh_report_only_captures`.
That pass already runs every two seconds and before every listing reply. For each entry: compute its restart offer with
the same generic call `advance_capture` uses; if it is `RestartOffer::Resume` and the entry's notification cell holds an
unresolved entry, call the guarded resolve for the kinds that resolve; if rows changed, reload the notification cell and
hint that sessions changed. Respect `may_record()` as `notify_session` does. Which kinds resolve comes from a
`NotificationKind` method beside `requires_no_identity` (HookSilent and ResumeWithdrawn), not from a match in shared
code; root `AGENTS.md` (Harness-specific code) and the map in `crates/farhelm-supervisor/src/agent_kind/mod.rs` forbid
`kind == X` over harnesses, and `RestartOffer::Resume` is already harness-neutral. A session whose only unresolved entry
is a non-resolving kind while its offer is Resume costs one zero-row indexed update per pass; do not add a latch for
that up front.

Resolving "never reported" on "the offer is Resume" (rather than on "the row holds a captured conversation", the inverse
of its insert guard) is a planner choice that follows the TODO's wording ("a late report arrives and Restart works").
Either is defensible; state the one you implement in SPEC_impl.md.

Do not reset the in-memory `hook_warned` latch: once a conversation is captured it is never erased within a launch, and
the store refuses the "never reported" kind then anyway, so a reset could never have an effect.

**Publishing order.** `reload_notification_cell` (`service/notifications.rs` ~260) reads the store outside any lock and
publishes when `newer_or_equal` (~294) accepts the snapshot, on the premise that the store only gains newer entries. An
in-place resolve breaks that: with the newest seq unchanged, a reload that read before the resolve can publish after one
that read after it, and the notification shows unresolved until the next recording. The realistic trigger is this plan's
main case, the tripwire recording "never reported" just as a late report resolves it. Order snapshots by (newest seq,
number of resolved entries): with the newest seq fixed, only resolutions can differ, and they only grow. Update the
guard's doc comment. A re-open raises the newest seq and is ordered correctly already.

**Wire.** `SessionNotification` in `crates/farhelm-proto/src/lib.rs` (~1089) gains `resolved: bool` with
`#[serde(default, skip_serializing_if = ...)]` for false. The helm needs no code: `SessionRow` flattens `SessionInfo`,
its cache compares the whole stored row JSON (so a flag flip on an unchanged seq is detected and pushed), and
`apply_notification_marks` (`crates/farhelm-helm/src/aggregate.rs`) works by seq, which suits both an in-place resolve
and a re-open above the read and cleared marks. Update the wire fixtures in
`crates/farhelm-helm/src/http_contract_tests.rs` and `crates/farhelm-ui/src/api/http_contract_tests.rs`, and any fixed
wire samples.

**UI** (`crates/farhelm-ui/src/lib.rs` ~495–530, `crates/farhelm-ui/src/list/bell.rs`). The UI's `SessionNotification`
gains the field; `unread_notifications()` excludes resolved entries; `NotificationList` does not give a resolved entry
the `new` class; resolved entries render greyed with a "resolved" marker. The bell's label and loudness follow from the
unread count.

**Tests (proposal).** Store: resolve, the row guard (a resolve with a stale conversation does nothing), re-open bumps
the seq and clears `resolved_at` without trimming, and uniqueness is kept. Capture and core: a late report resolves
"never reported" (extend the `a_silent_hook_*` tests in `service/capture.rs`); Grok withdraw, restore (resolves),
withdraw again (re-opens as newest) next to `grok_prepublication_binding_reconciles_and_file_loss_withdraws_resume` in
`service/core.rs`; the same for Codex, which has no notification test today; a notification recorded before a supervisor
restart resolves after it when the offer is Resume. The ordering guard: a unit test of the new comparison. UI: bell and
list unit tests next to `the_bell_name_escapes_the_title_and_counts_unread`. Playwright
`e2e/tests/notifications.spec.ts` (it stubs the listing): a resolved entry renders marked and does not make the bell
loud. No HookSilent re-open tests: that path is unreachable. Run the changed Playwright specs on Chromium and WebKit
through the recorder per root `AGENTS.md`.

**Size and stack.** Moderate. Suggested: PR 1 the store, the reconciliation step, the ordering guard and the proto field
(all the logic, invisible to users until the UI reads the flag); PR 2 the UI, docs, wire fixtures and the changelog
fragment (`feat`, kind `changed`, with the commit that changes what users see), plus the TODO removal.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-resolve-stale-notifications-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/resolve-stale-notifications/<nn>-<short-name>`.
- Shape the stack per the outline. Err on the side of bite-sized PRs, without churn: no code added in one PR and deleted
  in a later one. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits. The user-visible change is a `feat` and carries a changelog fragment under
  `releasing/changelog.d/` (kind `changed`) in the same commit, per root `AGENTS.md` (Releases and the changelog).
  Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge (`plans/AGENTS.md`).
- Changes to tests or fixtures follow `.agents/test-authoring.md`; run `python -B scripts/check-test-sleeps.py` per root
  `AGENTS.md` when Rust or browser tests change.
- The last code PR removes the TODO.md entry "Clear a session's notification once its problem goes away".

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, a nextest selection of the supervisor's store, `service::capture`,
`service::notifications` and the Codex/Grok capture tests in `service::core`, the helm's and the UI's http_contract
tests, the `farhelm-ui` crate's `list::bell` and row tests, `cargo check -p farhelm-ui --features desktop`, the changed
Playwright specs on Chromium and WebKit through the recorder, the website build for the docs page, and `dprint check` on
changed files. Say in the report which checks ran and why.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to TWO
fresh-context reviewers, as the user demands: a Claude Opus 5.5 agent at high effort, and a gpt-6-astra agent at high
effort (shelled out to the other harness if the executing one cannot reach that model natively). No review swarm. Both
get the same prompt, which carries the full charter, because the reviewer has nothing else:

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

Before implementing a substantial departure from the outline above (a transition hook in `advance_capture` or
`finish_reported_admission`, a helm change, a notification id or kind on the wire, a resolved timestamp shown to users;
these are examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run
a fresh-context review through galaxy-brain with this charter, supplying the request, the decisions above, this outline,
the current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled
out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the schema version taken, the re-open statement, the resolve guard, which offer
predicate resolves "never reported", the snapshot ordering, how a resolved entry looks, the PR split, and every reviewer
finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. The planner proposals in the outline are yours to change when the code argues otherwise. Not
covered, and a reason to block per `plans/AGENTS.md` (Executing one plan, step 10) after recording the concrete
tradeoff: removing notifications instead of marking them; a second entry per kind per launch; resolving the kinds that
have no resolving event or older launches' notifications; a protocol change or anything that breaks the upgrade path;
and a resolve that can land after a newer withdrawal.

## Done criterion

The plan is complete when its linear stack of draft PRs exists, satisfies the acceptance criteria, has passed the review
gate, and has removed the TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report
through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
