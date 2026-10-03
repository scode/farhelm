# Execute the 2026-10-02 boundary-check triage outcomes: five cheap defensive checks

Written against main at 9e1dde03 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out the triage outcomes recorded in root `TRIAGE_OUTCOMES.md` whose Execution field reads
`` planned in `plans/queue/triage-boundary-checks.md` ``, exactly as root `AGENTS.md` section "Execute triage outcomes"
prescribes: one reviewable commit, one stable bookmark and one draft PR per outcome, in a single linear stack, in this
order:

1. `list-ingress-id-validation-gap.md` (fix code)
2. `profile-body-accepts-unknown-fields.md` (fix code)
3. `provision-lock-map-grows-per-requested-id.md` (fix code)
4. `restart-with-skips-create-validation.md` (fix code)
5. `escape-token-clamp-too-short.md` (fix code)

Each outcome's `TRIAGE_OUTCOMES.md` entry (Assessment, Decision, Completion criteria) is the authoritative statement of
what that PR must do. Read the entry and its feedback file under `review_feedback_queue/` before starting the item. This
file adds the plan-time decisions and the mechanisms proposed to satisfy those entries; where the two seem to disagree,
the ledger entry wins and the disagreement is a DECISION to log (or a question, per Unattended fallback).

Acceptance criteria:

- 5 draft PRs exist, stacked in the order above (see PR discipline).
- Each PR does what its ledger entry's Completion criteria say, as refined by the plan-time decisions below.
- Each PR removes its feedback file and its line in `review_feedback_queue/INDEX.md`, and updates its own
  `TRIAGE_OUTCOMES.md` Execution field.
- Each PR has passed the review gate.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request:** "now use the planning system to plan execution of what we triaged", after a triage session on
2026-10-02 that recorded these outcomes (landed in #1475).

**The user's triage decision (2026-10-02),** recorded in each ledger entry's Decision field: "yes - when cheap. we don't
expend tons of complexity for defense in depth at every level, but reasonable straight-forward defensive checks are
encouraged." All five were checked and found cheap. If one turns out not to be, that is a question for the user, not a
reason to build more.

**The user's plan-time decisions (2026-10-02):**

- P1. Review gate: one fresh-context Opus 5.5 reviewer at high effort per PR, with the general charter below.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** are the per-item mechanisms below.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes;
Releases and the changelog; Harness-specific code; Sharing the machine with other agents; Agent scratch space; The live
install is off-limits), `plans/AGENTS.md` (Executing), `review_feedback_queue/AGENTS.md` (Lifecycle), and
`.agents/test-authoring.md` for test changes.

## Per-item outline

Line numbers drift; find the code by name. Each item is small; keep it small. Where a change has no effect for someone
running Farhelm, its changelog fragment is `kind: none` with the reason (for example, only a broken or hostile peer or a
third-party client could send the refused input); log the DECISION.

1. **One session-id check for both peer ingress points.** `fix:`.
   - Today `drain_sessions` (`crates/farhelm-helm/src/manager.rs`) checks only the reply cap, id length and duplicates,
     while create's `created_session_from` (`crates/farhelm-helm/src/client.rs`) also refuses empty and
     control-character ids; its doc comment wrongly says both apply the same rule. Supervisors mint UUIDs.
   - Proposal: generalize `session_cache::ensure_recordable_id` into one check (non-empty, within the length cap, no
     control characters, not `.` or `..`; a conservative character set such as `[A-Za-z0-9._~-]` is optional, log the
     DECISION) and call it from both. A list containing a bad id is refused whole, keeping the previous cache, as
     oversized and duplicate ids already are; dropping just the row would make the cache treat that session as ended.
   - UI: correct `encode_path_segment`'s doc comment and test comment in `crates/farhelm-ui/src/api.rs`, which claim
     `%2E` blocks dot-segment resolution (under the WHATWG URL Standard it does not). No behavior change there; the helm
     check is the boundary.
   - Tests: list replies with an empty id, a control character, `.` and `..` are refused and the cache is unchanged.
2. **Profile edits reject unknown fields.** `fix:`. Add `#[serde(deny_unknown_fields)]` to `ProfileSpec`
   (`crates/farhelm-helm/src/profiles.rs`), keeping `resume_template` optional: the e2e helper in
   `e2e/tests/helpers/fleet.ts` and a Rust e2e test omit it. Add a REST test showing a misspelled key is refused and the
   stored profile is unchanged.
3. **The provisioning lock map stops growing for unknown hosts.** `fix:`. `host_provision_lock` and
   `try_host_provision_lock` (`manager.rs`) insert an entry for any requested id before any existence check.
   - Proposal: remove an entry when its guard drops and nothing else holds or waits on it (strong count checked under
     the map's mutex, which is race-free because clones are only taken under that mutex). That bounds the map to locks
     in use, needs no store read, and leaves callers unchanged.
   - Correct the doc comment that claims the map is bounded by registered hosts. Test: lock many unregistered ids and
     check the map's size afterwards.
4. **Restart-with applies create's checks.** `fix:`. The restart-with branch of `restart_session`
   (`crates/farhelm-supervisor/src/service/core.rs`) skips `ensure_no_cwd_program` and `ensure_resume_template`, and the
   restart handler skips create's template element cap, while loading refuses a row failing those checks; SPEC_impl.md
   ("Restart-with backend wire and persistence") already says restart-with uses create's checks.
   - Apply both checks and the element cap before anything is stopped, mapping them and the existing argv and resolve
     errors (today untyped) to `ErrorKind::InvalidRequest`.
   - Test: a bundle whose template has `{conversation}` as its program is refused, the stored row is unchanged, and a
     fresh supervisor still loads its sessions.
5. **Shortening a host label never splits an escape token.** `fix:`. `back_off_from_split_escape`
   (`crates/farhelm-ui/src/menu_panel.rs`) assumes 8-character `<U+XXXX>` tokens, but tag characters U+E0000–U+E007F
   render as 9-character tokens such as `<U+E0041>`. Look back far enough for the longest token (`<U+10FFFF>`, 10
   characters) and back off to the nearest `<` (searching from the end: a complete token followed by a lone `<` must not
   match the earlier token's `<`). Fix both doc comments that claim eight characters, and test a cut inside a
   `<U+E00xx>` token.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-triage-boundary-checks-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and `TRIAGE_OUTCOMES.md` Execution fields on each PR) rather than starting over. If it does not exist, this is a fresh
start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/triage-boundary-checks/<nn>-<short-name>`.
- One outcome per commit, bookmark and draft PR, in the order listed in The goal. Never combine outcomes. Within this
  run, if a PR needs correcting, restructure it rather than stacking a correction on top.
- Commit messages and PR titles use Conventional Commits. The type reflects the user-visible effect, as each item below
  says. Every `fix:` or `feat:` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); `kind: none` with a one-line reason is right when nothing changes for
  someone running Farhelm. Validate with `python3 releasing/check-changelog.py format`.
- Each PR removes its feedback file and its line in `review_feedback_queue/INDEX.md`, and updates its own
  `TRIAGE_OUTCOMES.md` Execution field to `complete` with the jj change ID, bookmark and PR URL. Record the change ID
  and bookmark before creating the PR, then add the URL to the same change and push again; no separate bookkeeping PR.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression in each PR, through
`scripts/record-test-run.py` for Rust test execution (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Typical choices, not a checklist:

- PRs 1 to 3: `cargo fmt --all -- --check`, clippy on `farhelm-helm` (and `farhelm-ui` for PR 1's comment fix), and
  focused nextest selections for the new tests and the neighbouring tests in the touched modules.
- PR 4: the same on `farhelm-supervisor`, with `--tmux required` if the selection touches tmux-backed tests.
- PR 5: clippy and the `menu_panel` unit tests on `farhelm-ui`.
- Any PR that changes Rust or browser tests or their helpers: run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.
- A PR that only changes Markdown: `dprint check` on the changed files, nothing else.

### Review gate

Before finishing every PR, use the active galaxy-brain skill to delegate a review of that PR's changes. The user demands
the reviewer: a fresh-context agent on Opus 5.5 at high effort, with the general charter below. No review swarm. The
prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria for that PR: its `TRIAGE_OUTCOMES.md` entry, its item above, and
the plan-time decisions that apply to it. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Address what the reviewer finds before moving on;
where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a UI-side refusal of dot-only ids, making
`resume_template` a required key, a store read before every lock, validating ids anywhere beyond the two ingress points;
these are examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run
a fresh-context review through galaxy-brain with this charter, supplying the ledger entry, the user decisions above,
this outline, the current diff and the proposed departure (what changed, why it is necessary, and which simpler
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
alternatives considered: in particular the session-id character set, the lock map's pruning approach, and each changelog
kind, each changelog kind, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. If an item needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing
one plan, step 10). Because the PRs form one linear stack, later items sit on top of the blocked one: finish the PRs
before it, record the question, and close the plan as blocked rather than building past it. If current code or specs
have moved so that a recorded decision no longer applies, that is a question for the user too, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself.

## Done criterion

The plan is complete when all 5 draft PRs exist as one linear stack, each satisfies its ledger entry's Completion
criteria as refined by the plan-time decisions, every PR passed the review gate, and each PR has updated its own
`TRIAGE_OUTCOMES.md` Execution field and removed its queue item. Open, not merged: merging happens only after the
maintainer has reviewed this plan's report. If a `## Decisions` section exists, its latest entry must also be satisfied.
Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
