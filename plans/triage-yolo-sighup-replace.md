# Execute the third 2026-10-01 triage batch: YOLO detection, SIGHUP shutdown, Replace liveness

Written against main at 1a6cf4c on 2026-10-01. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Carry out the 9 triage outcomes recorded in root `TRIAGE_OUTCOMES.md` whose Execution field reads
`` planned in `plans/triage-yolo-sighup-replace.md` ``, exactly as root `AGENTS.md` section "Execute triage outcomes"
prescribes: one reviewable commit, one stable bookmark and one draft PR per outcome, in a single linear stack. The stack
order departs from ledger order on purpose: the four outcomes that change how a command line is recognized as YOLO go
first and in this sequence, because they edit the same classifier (`crates/farhelm-proto/src/yolo.rs`) and the same
SPEC.md paragraph, and the later ones build on the best-effort wording the first one writes; the header Replace fix goes
before the sidebar one because the sidebar outcome's spec work is conditional on the header's.

1. `yolo-guard-misses-env-prefix.md` (fix spec+code)
2. `yolo-guard-misses-codex-option-form.md` (fix code)
3. `yolo-guard-misses-equivalent-spellings.md` (fix spec+code)
4. `yolo-guard-skips-resume-template.md` (fix spec)
5. `yolo-guard-fails-open-without-row.md` (fix code)
6. `yolo-safe-survives-identity-adoption.md` (fix spec+code)
7. `sighup-skips-orderly-shutdown.md` (fix code)
8. `header-replace-recomputes-alive.md` (fix spec+code)
9. `sidebar-replace-recomputes-alive.md` (fix spec+code)

Each outcome's `TRIAGE_OUTCOMES.md` entry (Assessment, Decision, Completion criteria) is the authoritative statement of
what that PR must do. Read the entry and its feedback file under `review_feedback_queue/` before starting the item. This
file adds the plan-time decisions and the mechanisms chosen to satisfy those entries; where the two seem to disagree,
the ledger entry wins and the disagreement is a DECISION to log (or a question, per Unattended fallback).

Acceptance criteria:

- 9 draft PRs exist, one per outcome, stacked in the order above on top of the plan stack's tip (see PR discipline).
- Each PR does what its ledger entry's Completion criteria say, as refined by the plan-time decisions below, removes its
  feedback file and its line in `review_feedback_queue/INDEX.md`, and updates its own `TRIAGE_OUTCOMES.md` Execution
  field to `complete` with the jj change ID, bookmark and PR URL (record the change ID and bookmark before creating the
  PR, then add the URL to the same change and push again; no separate bookkeeping PR).
- Every PR has passed the review gate.
- PR 9 also changes this plan's line in `plans/INDEX.md` to `[executed]`. Do not delete this plan file.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request:** "use the planning system to queue up execution of the items we just triaged. opus 5.5 reviewing
agent."

**The user's triage decisions (2026-10-01),** recorded in full in each ledger entry's Decision field. In short:

- YOLO detection for custom launches (raw command lines and profile invocations) is best effort: cover as many
  reasonable, documented shapes as possible, and never let the spec claim complete detection for arbitrary command
  lines. Structured launches stay exact.
- Item 1: look past an `env NAME=value` prefix; spec states the best-effort rule. A `Doc todo` entry in TODO.md already
  tracks telling users about the limitation; this plan does not write user docs for it.
- Item 2: recognize Codex's `-a never` with `-s danger-full-access` in any spelling; either alone does not count.
- Item 3: add Cursor's `-f`. Agents installed or launched under other program names are out of scope (a declared Pi kind
  under another name included), stated in the spec. `cursor-agent` means Cursor, `grok` means Grok, and the name `agent`
  is not interpreted ("'agent' is just too freaking general of a name anyway"). Farhelm's own Cursor launches switch to
  `cursor-agent` so the built-in `cursor-yolo` profile stays guarded.
- Item 4: spec only. Only the start command of a custom launch is classified; a separate resume command is not. A
  `Near term` TODO entry already tracks re-examining how launches work; this plan does not act on it.
- Item 5: refuse a missing host row as host-not-found. The wider create-versus-removal race is discarded: add no locking
  between create and host removal.
- Item 6: clear the "start YOLO sessions without asking" setting on adopt only; the stale-dialog and never-contacted-row
  variants are discarded.
- Item 7: handle SIGHUP through the orderly shutdown, give the tmux clients their own process group, correct `BUGS.md`.
- Items 8 and 9: keep the "nothing alive" answer from the prompt the user confirmed. Fix only if easy and low in
  complexity. Spec principle: a single GUI attached to the helm is the supported user surface and several concurrent
  GUIs are best effort, while the `farhelm` command line and the agent skill are a fully supported primary surface
  alongside the UI, including concurrently with it. A `Maybe later` TODO entry already tracks considering a strict
  one-UI rule; this plan does not act on it.

**The user's plan-time decisions (2026-10-01):**

- P1. Review gate: a fresh-context Opus 5.5 agent at high effort with the general review charter, for all 9 PRs,
  including the spec-only PR 4. No review swarm.
- P2. Item 1, `env` followed by an option (`env -i …`, `env -u NAME …`): the guard does not interpret the option. It
  counts the launch as YOLO when any later word's basename is a program the classifier knows (a vendor CLI from its
  tables, or a sole-YOLO program such as `pi`), and as not YOLO otherwise, so ordinary non-agent commands are not
  prompted. This refines the ledger's "treated as YOLO" wording. The sidebar badge does not apply this rule; it badges
  only a flag it actually recognizes.
- P3. Item 3: accept that sessions already stored with a launch of `agent --force` lose their sidebar YOLO badge, and
  that cloning or replacing from such an older raw session is not YOLO-checked. No carve-out for them. Say so in the
  changelog fragment.
- P4 (planner decision, from the user's "not assume anything about `agent`"): the sidebar's Cursor harness mark
  (`known_harness` in `crates/farhelm-ui/src/list/row.rs`) also stops mapping the program name `agent` to Cursor and
  maps `cursor-agent` instead. Older sessions stored as `agent` lose the Cursor mark along with the YOLO badge (P3).
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes;
Releases and the changelog; Harness-specific code; Sharing the machine with other agents; Agent scratch space; The live
install is off-limits), `plans/AGENTS.md` (Executing), `review_feedback_queue/AGENTS.md` (Lifecycle),
`website/AGENTS.md` for any docs-site page touched, and `.agents/test-authoring.md` for test changes.

**Planner proposals** are the per-item mechanisms below. A fresh-context planning review checked them for unnecessary
scope; its findings are folded in.

## Per-item outline

Line numbers are approximate; find the code by name.

1. **`env NAME=value` prefix.**
   - Move the supervisor's rule for skipping a leading simple `env NAME=value …` prefix (`effective_program_index` in
     `crates/farhelm-supervisor/src/agent_kind/mod.rs`) into `farhelm-proto` so the supervisor, `argv_is_yolo` and
     `invocation_marker` (`crates/farhelm-proto/src/yolo.rs`) share one copy, and have the supervisor call the shared
     one. Read root `AGENTS.md` "Harness-specific code" first: this rule is not per-harness, so it belongs beside the
     classifier, not in the per-harness map.
   - Classify the program found past the prefix. For `env` followed by an option, apply P2 in the guard. Derive P2's set
     of known programs from the classifier's own tables (vendor flags, option values, guard-only flags, sole-YOLO
     programs), never from a hand-written list, so items 2 and 3 cannot drift from it.
   - The supervisor's callers (`goose.rs`, `omp.rs`, `pi.rs`, `agent_kind/mod.rs`) must behave exactly as before. Today
     `effective_program_index` takes the program's file name slightly differently from the classifier's
     `program_basename` (they differ on a token ending in `/`); keep the supervisor's behavior, and let the existing
     supervisor tests prove it.
   - Tests: `env A=1 claude --dangerously-skip-permissions`, `/usr/bin/env A=1 codex --yolo`, `env A=b pi`, an
     `env -i … claude --dangerously-skip-permissions` case (YOLO), and an `env -u FOO ./build.sh` case (not YOLO), plus
     the badge for the simple-prefix cases.
   - Spec: SPEC.md's YOLO-launch paragraph (around "A launch counts as YOLO when …") says recognition of custom command
     lines is best effort: common documented shapes are covered, including behind an `env NAME=value` prefix, but
     arbitrary wrappers (scripts, `sh -c`) are not guaranteed to be detected, while structured launches are exact.
     Update SPEC_impl.md wherever it describes the classifier, if it does.
2. **Codex option form.** Add Codex's never-ask approval policy together with the `danger-full-access` sandbox, in any
   of the `-a`/`--ask-for-approval`, `-s`/`--sandbox` and `=` spellings and either order, to the guard's YOLO
   classification and to the badge. The guard needs a rule that both options are present, which the single-value
   `YOLO_OPTION_VALUES` table cannot express: add it as table data keyed by program, not as an `if basename == "codex"`
   in shared code (root `AGENTS.md`, Harness-specific code). The badge's switch parser currently skips those options'
   values without reading them, so badging the pair needs value-reading there, mapped to the existing no-sandbox marker
   rather than a new one. Include the `-c`/`--config` spellings only if they fit the same mechanism without a TOML
   parser; otherwise log the omission as a DECISION. Tests per spelling, the two lone forms as not YOLO, and a guard
   test refusing such a create on a sensitive host.
3. **Cursor and the name `agent`.**
   - Remove `agent` from the classifier tables; add `-f` to `cursor-agent`'s YOLO flags (guard and badge).
   - Launch Cursor as `cursor-agent`: the built-in `cursor` and `cursor-yolo` profiles (`builtin_profiles` in
     `crates/farhelm-helm/src/store.rs`) and the structured harness's program (`LaunchHarness::Cursor` in
     `crates/farhelm-helm/src/launches.rs`). Built-in profiles live in code: `builtin_profiles()` is merged in when the
     catalog is read and `profile(id)` checks built-ins first, and no stored starter row was ever a Cursor one, so
     changing `builtin_profiles` needs no migration. Stored sessions keep their recorded launch (P3). The built-in ids
     `builtin-cursor` and `builtin-cursor-yolo` stay the same, so UI code keyed on them needs no change.
   - Apply P4 to the sidebar's harness mark.
   - Update tests and fixtures that hard-code Cursor as `agent`: `launches.rs` and its test, the
     `"agent" |
     "cursor-agent"` arm of `invocation_switches` in `yolo.rs`, the built-in profile tests in
     `store.rs`, the `agent` wrapper fixture in `crates/farhelm/tests/e2e/structured_launches.rs`, and UI test fixtures.
     Most `"agent"` strings in the repository are Farhelm's own `farhelm agent` subcommand and are unrelated; change
     only Cursor ones.
   - OMP's `--auto-approve` and Grok's `--permission-mode bypassPermissions` are explicitly not part of this decision
     (ledger); do not add them while editing the tables.
   - Spec: SPEC.md's Cursor section names `cursor-agent`; the YOLO paragraph says custom command lines are recognized by
     each vendor's standard program name (`cursor-agent` for Cursor, `grok` for Grok, `pi` for Pi, and so on), that the
     generic name `agent` is not interpreted, and that agents installed or launched under other names are not detected.
     SPEC_impl.md (around its Cursor harness description) also says the built-ins invoke `agent` and `agent --force`;
     update it to match.
   - Docs site: `website/src/content/docs/docs/agents/cursor.md` describes the `agent` launcher and the name clash;
     update it to `cursor-agent` per `website/AGENTS.md`.
   - Changelog fragment: Cursor is now launched as `cursor-agent`, which must be on the host's PATH, plus the P3 caveat.
4. **Resume command not classified.** SPEC.md's YOLO paragraph: for custom launches only the start command is
   classified; a separate resume command is not checked, and a plain Resume or Restart that runs it is not asked.
   Consistent with item 1's wording. No code. `docs:` type, no changelog fragment.
5. **Missing host row.** In `check` (`crates/farhelm-helm/src/yolo_guard.rs`), refuse a missing row with the helm's
   existing host-not-found error rather than OK, and correct the comment claiming routing refuses on its own. Two
   not-found errors exist (`HostStoreError::NotFound` and `ManagerError::NoSuchHost`); use whichever the create path
   already returns for an unknown host, so the command line, agents and the browser see the same text. Unit test in the
   existing YOLO-guard tests (`crates/farhelm-helm/src/sessions_tests.rs` or beside the guard).
6. **Adopt clears the YOLO setting.** Clear `yolo_safe` in `adopt_identity`'s existing `UPDATE hosts …` statement
   (`crates/farhelm-helm/src/store.rs`); say in the adopt prompt (`crates/farhelm-ui/src/hosts.rs`) that YOLO launches
   will ask again after adopting; add the SPEC.md sentence to the host settings paragraph. Store test: mark safe, adopt
   a different identity, the row is no longer safe.
7. **SIGHUP.**
   - Add a hangup listener in `run` (`crates/farhelm-supervisor/src/service/core.rs`, beside the SIGTERM and SIGINT
     ones) routed into the same orderly shutdown; update `run`'s doc comment.
   - Spawn the supervisor's long-lived tmux clients in their own process group (`process_group(0)`, as
     `bounded_command.rs` and `launch.rs` already do), so a terminal hangup or Ctrl-C to the supervisor's group does not
     reach them: the output clients (`crates/farhelm-supervisor/src/tmux/stream.rs`), the sink clients (`tmux/sink.rs`)
     and the input clients (`tmux/input.rs`, which the finding missed but which are attached clients too). Set it at
     those three spawn sites, not in the shared tmux `command()` helper, which also runs short one-off commands.
   - Leave the desktop app's managed-supervisor spawn alone (the ledger makes it optional) and log that as a DECISION.
   - Correct `BUGS.md`'s "Abrupt supervisor death" entry so its list of deaths that skip the orderly path matches.
   - Test: a focused test that a SIGHUP to the supervisor's process group runs the orderly shutdown, reusing the
     existing orderly-stop test's seams (`supervisor_stop.rs`). That test starts the supervisor in the test runner's own
     process group, so the new case must start the supervisor in a process group of its own and signal that group;
     signalling the shared group would hang up the test runner. Do not try to reproduce the tmux abort.
8. **Header Replace keeps the confirmed answer.**
   - In `crates/farhelm-ui/src/session_view.rs`, the `replace` closure takes the nothing-alive value, together with the
     source fields it sends (the ledger's "with the source fields"), as parameters; each prompt that can start Replace
     (the header prompt and the interrupted-session card) captures them from the render that drew its warning, the way
     header Delete already does; `replace_yolo` stores them beside the `YoloAsk`, and both YOLO buttons pass the stored
     values. If carrying the source fields turns out unnecessary, log why as a DECISION.
   - Browser regression in `e2e/` (the existing `replace.spec.ts` and `yolo-guard.spec.ts` show the setup), run through
     the recorder on Chromium and WebKit per root `AGENTS.md`.
   - Spec: add the single-GUI principle (the items 8 and 9 triage decision above) to SPEC.md without contradicting the
     session view section's multi-client rules (one attached client per session, takeover, displaced clients) or the
     existing "desktop app is the primary supported surface" statement under "Signing in again"; state that the
     `farhelm` command line and the agent skill are a fully supported primary surface alongside the UI, concurrently
     included.
   - If the fix turns out not to be easy, block per Unattended fallback rather than adding machinery.
9. **Sidebar Replace keeps the confirmed answer.** In `crates/farhelm-ui/src/list/view.rs`, capture the nothing-alive
   value from the row the confirm prompt was drawn from (`confirm_replace` / `do_replace`), carry it in `yolo_replace`'s
   state, send the stored value from both YOLO buttons, and make a row missing from the list stop producing an unguarded
   delete. Correct `do_replace`'s comment. Browser regression as in item 8. No further spec change if PR 8 added the
   principle. This PR marks this plan's `plans/INDEX.md` line `[executed]`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-triage-yolo-sighup-replace-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and `TRIAGE_OUTCOMES.md` Execution fields on each PR) rather than starting over. If it does not exist, this is a fresh
start.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain.

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

- Use the `jjstack` skill. The stack's base is not main but the tip of the plan stack, set up per `plans/AGENTS.md`
  (Executing, step 4). PRs already in the plan stack, from earlier plans or an earlier blocked run of this one, are the
  base and are not rewritten.
- One outcome per commit, bookmark and draft PR, in the order listed in The goal. Never combine two outcomes, and never
  split a fix spec+code outcome across PRs. Within this run, if a PR needs correcting, restructure it rather than
  stacking a correction on top.
- Commit messages and PR titles use Conventional Commits. The type reflects the user-visible effect: `fix:` for items 1,
  2, 3, 5, 6, 7, 8 and 9; `docs:` for item 4. Every `fix:` PR adds a changelog fragment under `releasing/changelog.d/`
  in the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing the plan stack is the user's job.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression in each PR, through
`scripts/record-test-run.py` for Rust test execution (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Typical choices, not a checklist:

- PRs 1 to 3: `cargo fmt --all -- --check`, clippy, and focused nextest selections for the classifier (`farhelm-proto`),
  the YOLO guard and the session-create tests in `farhelm-helm`, and for PR 1 the supervisor tests around
  `effective_program_index`; for PR 3 also the store tests on built-in profiles, the UI crate's tests touching the
  Cursor profile ids, the relevant e2e structured-launch tests, and the website build for the docs page.
- PR 4: `dprint check` on the changed files, nothing else.
- PRs 5 and 6: fmt, clippy on `farhelm-helm` (and `cargo check -p farhelm-ui` for PR 6's wording), focused nextest for
  the guard and store tests.
- PR 7: fmt, clippy on `farhelm-supervisor` and `cargo clippy -p farhelm --bins -- -D warnings`, and focused nextest for
  the shutdown and tmux client tests with the pinned tmux on PATH.
- PRs 8 and 9: `cargo check -p farhelm-ui`, the UI unit tests touching these views, and the new and existing Replace and
  YOLO-guard Playwright specs on Chromium and WebKit through the recorder, after the builds root `AGENTS.md` names.
- Any PR that changes Rust or browser tests or their helpers: run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes. The user demands
this reviewer: a fresh-context agent on Opus 5.5 at high effort, with no review swarm (P1). The prompt carries the full
charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria for that PR: its `TRIAGE_OUTCOMES.md` entry, its item above, and
the plan-time decisions that apply to it. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Address what the reviewer finds before moving on. Do
not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a new subsystem, registry, recovery protocol,
compatibility layer, or anything else the ledger entry did not imply; these are examples, not a blacklist), and whenever
the same component has needed repeated corrective review rounds, run a fresh-context review through galaxy-brain with
this charter, supplying the ledger entry, the user decisions above, this outline, the current diff and the proposed
departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular where the shared `env` rule lives (item 1), whether the Codex `-c` forms were
included (item 2), the spec wording for items 1, 3, 4 and 8, and leaving the desktop app's managed-supervisor spawn
alone (item 7). The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. Items 8 and 9 carry an agreed stop: if the fix is not easy, block rather than add machinery. If an item
needs such a decision, record the concrete tradeoff, and block per `plans/AGENTS.md` (Executing, step 7). Because the
PRs form one linear stack, later items sit on top of the blocked one: finish the PRs before it, record the question, and
close the plan as blocked rather than building past it. If current code or specs have moved so that a recorded decision
no longer applies, that is a question for the user too, per root `AGENTS.md` (Execute triage outcomes); never re-triage
an item yourself.

## Done criterion

The plan is complete when all 9 draft PRs exist as one linear stack on the plan stack's tip, each satisfies its ledger
entry's Completion criteria as refined by the plan-time decisions, each has passed the review gate, each has updated its
own `TRIAGE_OUTCOMES.md` Execution field and removed its queue item, and PR 9 has marked this plan's `plans/INDEX.md`
line `[executed]`. Open, not merged: merging is the user's job. Then close the plan per `plans/AGENTS.md` (Executing,
step 8): write its report, write a closing entry in its log, and stop the watchdog.
