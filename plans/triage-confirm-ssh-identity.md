# Execute the fifth 2026-10-01 triage batch: binding confirmations, ssh overrides, report-only identity

Written against main at f3a9e7b on 2026-10-01. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `plans/triage-restart-takeover-update.md`: item 18 builds on that plan's item 13 (the probe's
registration on a helm-owned task), and item 15 mirrors that plan's item 1 (Codex refusing to derive a resume command).
It also builds on `plans/triage-yolo-sighup-replace.md` items 8 and 9, which make Replace keep the "nothing alive"
answer its prompt showed; items 1 and 2 below extend the same idea to Delete, Restart and "Replace with".

## The goal

Carry out the 25 triage outcomes recorded in root `TRIAGE_OUTCOMES.md` whose Execution field reads
`` planned in `plans/triage-confirm-ssh-identity.md` ``, exactly as root `AGENTS.md` section "Execute triage outcomes"
prescribes: one reviewable commit, one stable bookmark and one draft PR per outcome, in a single linear stack, in ledger
order. There is one agreed exception: the two ssh outcomes share one PR, because they are the same spec sentence and the
same few ssh options. That makes 24 PRs:

1. `confirmed-nothing-alive-prompt-kills-live-agent.md` (fix spec+code)
2. `replace-with-kills-running-source-unwarned.md` (fix spec+code)
3. `claude-scan-claims-foreign-record.md` (other: a spec change)
4. `claude-scan-budget-never-settles.md` (discard)
5. `claude-capture-warns-forever.md` (discard)
6. `ssh-forwarding-inherited.md` and `ssh-config-remotecommand-blocks-host.md` (fix spec+code, one PR)
7. `merged-list-crowded-by-one-host.md` (fix spec)
8. `attach-reports-generic-timeout.md` (fix code, gated)
9. `folder-picker-skips-symlinks.md` (fix code, gated)
10. `tilde-in-remote-path-fields.md` (fix code, gated)
11. `drop-on-hidden-terminal-navigates-away.md` (fix code, gated)
12. `partial-release-download-left-behind.md` (fix code, gated)
13. `incarnation-counter-restarts-per-process.md` (fix code, gated)
14. `sessions-changed-hint-unthrottled.md` (fix spec+code)
15. `claude-resume-template-selector-collision.md` (fix spec+code)
16. `omp-corridor-uncounted-pane-runtime.md` (fix code, gated with its own fallback)
17. `process-snapshot-requires-supervisor-witness.md` (fix code)
18. `probe-cancellation-leaves-helper-processes.md` (fix code)
19. `pi-pointer-overrides-user-prompt.md` (discard)
20. `refresh-starved-by-seeds.md` (discard)
21. `env-wrapper-hides-command-not-found.md` (discard)
22. `event-feed-cap-refusal-invisible.md` (discard)
23. `terminal-font-promise-leak.md` (discard)
24. `desktop-copy-fallback-never-runs.md` (discard)

`claude-clear-report-dropped-on-claim-timeout.md`, decided in the same triage session, is not part of this plan: at
planning time the user promoted it to a TODO.md `Near term` entry instead (its ledger entry records why). Leave it and
its feedback file alone.

Each outcome's `TRIAGE_OUTCOMES.md` entry (Assessment, Decision, any plan-time Decision refinement, Completion criteria)
is the authoritative statement of what that PR must do. Read the entry and its feedback file under
`review_feedback_queue/` before starting the item. This file adds the plan-time decisions and the mechanisms proposed to
satisfy those entries; where the two seem to disagree, the ledger entry wins and the disagreement is a DECISION to log
(or a question, per Unattended fallback).

Acceptance criteria:

- 24 draft PRs exist, stacked in the order above on top of the plan stack's tip (see PR discipline).
- Each PR does what its ledger entry's (or entries') Completion criteria say, as refined by the plan-time decisions, or,
  for a gated item whose gate tripped, what the complexity gate below says instead.
- Each PR removes its feedback file(s) and line(s) in `review_feedback_queue/INDEX.md`, except a gated item whose gate
  tripped, which keeps its feedback file (see Complexity gate).
- Each PR updates its own `TRIAGE_OUTCOMES.md` Execution field(s): `complete` with the jj change ID, bookmark and PR
  URL, or, for a tripped gate, the deferral the complexity gate describes. Record the change ID and bookmark before
  creating the PR, then add the URL to the same change and push again; no separate bookkeeping PR.
- Every PR other than a pure discard has passed the review gate (both reviewers).
- PR 24 also changes this plan's line in `plans/INDEX.md` to `[executed]`. Do not delete this plan file.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request:** "use the planning system to schedule execution of the items we have triaged above".

**The user's triage decisions (2026-10-01),** recorded in full in each ledger entry's Decision field. The principles
that shape the work most:

- Items 1 and 2: a destructive confirmation authorizes only what the prompt the user answered said would happen; by
  default Farhelm does not accept races where a confirmation is applied to a state the user was not shown.
- Item 3 (and the discards 4 and 5): Farhelm identifies an agent's conversation only from the harness's own explicit
  report; heuristic fallbacks are not supported. Removing the Claude record scan itself is TODO.md's `Near term` entry
  "Remove heuristic conversation-identity fallbacks", not part of this plan. Findings true only because that code still
  exists are discarded.
- Item 6: Farhelm's own ssh connections never forward the agent, X11 or ports and ignore the user's `RemoteCommand`,
  whatever the user's ssh config says; the config still governs reaching the host.
- Items 7 and 14: removing a misbehaving host is the remedy for effects like cluttering the UI; only breaking the helm
  or affecting other hosts' security must be prevented. A misbehaving host degrading the helm's performance or
  availability is accepted when it cannot easily be avoided; Farhelm avoids it where it reasonably can, without
  elaborate complexity.
- Items 8 to 13: fix, behind the user's complexity gate (below).
- Item 15: refuse at creation, like Codex and Grok, chosen by the user over allowing the launch and withholding Resume.
- Item 16: fix with a complexity gate; if it turns out complicated, abandon it and record a `Near term` TODO.md entry
  instead.
- Item 17: strictly the case of the process snapshot not seeing the supervisor itself; not an opening to address
  processes that hide from discovery.

**The user's plan-time decisions (2026-10-01),** also folded into the affected ledger entries as plan-time Decision
refinements:

- P1. Review gate: two independent fresh-context reviewers per PR, one on Opus 5.5 at high effort and one on gpt-6-astra
  at high effort, both with the general review charter. No review swarm. Pure discard PRs (4, 5, 19 to 24) get no
  review.
- P2. The two ssh outcomes share one PR (the user left the PR count to the planner).
- P3. Complexity gate for items 8 to 13: if an item turns into a significant complexity increase or a refactor, do not
  fix it. Instead promote it to a `Near term` TODO.md entry to revisit, pointing at its review feedback file and saying
  why the gate was hit. Item 16 keeps its own fallback from the ledger.
- P4. Items 1 and 2: add one narrower precondition, "only if the agent has ended", beside `only_if_nothing_alive` on the
  Delete and Replace requests. A tab opened between the prompt and the click is still closed; that is accepted.
- P5. Item 2: "refused" keeps Replace's existing create-then-delete semantics: the replacement is created and a source
  that no longer matches what the launcher showed is kept, with the existing both-sessions-exist error.
- P6. `claude-clear-report-dropped-on-claim-timeout.md` was promoted to a TODO entry and is out of this plan.
- P7. Item 10: a bare program name in the remote farhelm field stays valid (planner clarification of an existing tested
  form, recorded in the ledger).
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes;
Releases and the changelog; Harness-specific code; Sharing the machine with other agents; Agent scratch space; The live
install is off-limits), `plans/AGENTS.md` (Executing), `review_feedback_queue/AGENTS.md` (Lifecycle), and
`.agents/test-authoring.md` for test changes.

**Planner proposals** are the per-item mechanisms below. A fresh-context planning review checked them for unnecessary
scope and against the code; its findings are folded in.

## Complexity gate (P3)

Applies to items 8 to 13. "Significant" means the fix needs a new subsystem, new persistent state, a cross-crate
refactor, or more than a small focused diff plus its test; when in doubt, it tripped. When it trips:

- Do not change product code for that item. The item's PR (still its own commit, bookmark and draft PR, so the stack
  stays one PR per outcome) adds a `Near term` entry to TODO.md in that file's style: a bold handle, what goes wrong for
  the user, why the fix is bigger than it looked (what you found), and a pointer to the feedback file
  (`review_feedback_queue/<name>.md`) and its `TRIAGE_OUTCOMES.md` heading. TODO.md's `Near term` entry "Apply identity
  reports that arrive while the session's record is busy" is an example of the shape.
- Keep the feedback file and its index line; the TODO entry points at it.
- Set the item's Execution field to `deferred` with a sentence naming the TODO entry and why the gate tripped, plus the
  change ID, bookmark and PR URL. `deferred` is a ledger state this triage batch introduced: the outcome was not
  executed and now lives as a TODO entry.
- The PR type is `docs:`; no changelog fragment. It still gets the review gate.
- Log it as a DECISION and list it in the plan's final report.

Item 16 differs only in what the ledger says: its TODO entry replaces the fix and the feedback file and its index line
are removed in the same change.

## Per-item outline

Line numbers drift; find the code by name.

1. **Delete and Restart prompts bind to what they showed.**
   - Wire (P4): add one precondition, for example `only_if_agent_ended`, following `only_if_nothing_alive`'s pattern
     (serde default false, not serialized when false), on proto `ControlMsg::DeleteSession`, the helm's `DeleteQuery`
     and `ReplaceReq` (`crates/farhelm-helm/src/sessions.rs`), and the UI's `api::delete_session`/`delete_url`. The
     supervisor checks it with the agent half of `still_alive_for_delete`
     (`crates/farhelm-supervisor/src/service/core.rs`). This field is authorized; it is not a scope reassessment.
   - The UI picks the level from the prompt the user answered: nothing alive sends `only_if_nothing_alive`, a tabs-only
     warning sends the new precondition, a warning that the agent runs sends neither. Apply it to the sidebar Delete
     prompt (`list/row.rs`, confirm wired through `on_confirm_delete` in `list/view.rs`) and to header Delete in
     `crates/farhelm-ui/src/session_view.rs`, which today sends only `only_if_nothing_alive`. Capture the level from the
     render that drew the prompt, as header Delete already does for its flag.
   - Header Restart: `confirm_restart(true, …)` in `session_view.rs` sends `stop_if_running: true` from a prompt that
     may have drifted; send it only when the answered prompt offered to stop a working agent.
   - Spec: state the principle generally in SPEC.md "Lifecycle operations": a destructive confirmation authorizes only
     what the prompt the user answered said; the request carries the matching precondition, and the server refuses when
     the current state exceeds it, so the next attempt asks again. Name the accepted residual (a tab opened between
     prompt and click is closed). Keep it consistent with the single-GUI principle `plans/triage-yolo-sighup-replace.md`
     item 8 adds (the CLI and agents are a fully supported concurrent surface, which is exactly why these races are
     reachable).
   - Tests: UI tests for a prompt whose wording drifted before the click; supervisor tests for the new precondition; the
     forwarding test in the helm's `sessions_tests.rs`.
2. **"Replace with" warns and binds.** `on_replace_with` (`list/view.rs`) opens the launcher straight from the row menu,
   and `replace_session_with` (`crates/farhelm-ui/src/api.rs`) and the fresh-checkout launch (`api::fresh_create_body` /
   `api::submit_fresh_create`) omit any precondition. Show Replace's existing consequence text in the launcher
   (`list/create_form.rs`) whenever the source has anything alive, and send the precondition level matching what the
   launcher displayed at launch time (P4). For the fresh-checkout launch, fix the precondition into its retained,
   replayable payload. Per P5, a source that no longer matches is kept and the user sees the existing
   both-sessions-exist error; there is no liveness check before the create. SPEC.md's "Replace with" text says so.
   Tests: a running source, and a source restarted while the launcher was open (the source survives and the user is
   told).
3. **Spec: identity only from explicit reports.** SPEC.md "Durability and resume" currently says identity is "scanned
   from the outside … otherwise" and "Claude retains scanning as its fallback". Replace that with the principle from the
   ledger, mention the Claude record scan only as existing behavior pending removal under TODO.md's `Near term` entry,
   and align the matching SPEC_impl.md capture text. Spec text only; `docs:`.
4. and 5. **Discards.** Remove the feedback file and its index line; no code or spec change. `docs:`. No review.
5. **ssh overrides.** In `ssh_base_args` (`crates/farhelm-helm/src/ssh.rs`, the only ssh argument builder, used by both
   the supervisor connection and provisioning), both branches add
   `-o ForwardAgent=no -o ForwardX11=no
   -o ClearAllForwardings=yes -o RemoteCommand=none -o RequestTTY=no`. Prove
   that `ClearAllForwardings` leaves `ProxyJump` working with a real connection through a jump on localhost (a jump runs
   as a separate `ssh -W` child, so `ssh -G` alone cannot show it), and log the result; if it breaks ProxyJump, drop
   that option and rely on the explicit `ForwardAgent`/`ForwardX11` settings, and log the DECISION. Update the argument
   tests. Spec: SPEC.md's Security text about provisioning riding the user's ssh access gains the override sentence, and
   SPEC_impl.md's list of honored ssh features stops naming agent forwarding. Both feedback files go.
6. **Spec: crowding is accepted.** SPEC.md "Remote input, session defaults, and availability" generalizes the
   session-ownership carve-out as the ledger says. Spec text only; `docs:`.
7. **Attach names the real state.** The attach wait (`ProvisioningAction::AttachSupervisor` in
   `crates/farhelm-helm/src/provisioning/service.rs`) nudges the connection with `retry_now_with_fresh_window` and then
   polls `manager.status(host)`; the nudge is asynchronous and the incarnation number does not change between two
   refusals, so the first reads can still show the refusal from before the install or update. Stop early only on a
   version skew, identity mismatch, unverified identity or duplicate state observed after the nudge took effect (for
   example after the fresh window's `Connecting` was seen), reporting the state and its remedy: reuse
   `require_update_trusted`'s wording for mismatch and duplicate and the skew state's existing `remediation` text, and
   write a short message for unverified identity. Tests: each state, and a host in skew before the update and connected
   after it (must succeed). Gated.
8. **Folder picker follows symlinks.** In `browse_directory_blocking` (`crates/farhelm-supervisor/src/service/core.rs`),
   for entries that are symlinks only, follow the link and keep it when the target is a directory; skip an entry whose
   type cannot be read instead of failing the listing. Do not stat ordinary entries. Test with a symlinked folder and a
   dangling link. Gated.
9. **Absolute remote paths.** Per P7, in `remote_farhelm_is_usable` and the state dir check
   (`crates/farhelm-helm/src/store.rs`): the remote farhelm field refuses a leading `~` and a relative value containing
   `/` but keeps a bare program name; the remote state dir refuses anything non-absolute. Messages ask for an absolute
   path. Extend `remote_farhelm_values_are_validated_at_registration`. The hosts panel
   (`crates/farhelm-ui/src/hosts.rs`) shows the refusal as it shows other validation errors. Check that
   `--ensure-hosts`, which uses the same registration path, does not turn an already-stored `~` value into a helm start
   failure; if it would, make that path report the host as needing correction instead, and log the DECISION. Gated.
10. **Stray drops never navigate.** A page-wide `dragover`/`drop` handler cancels the browser default, in an existing
    asset that already owns page-level listeners if one fits; a drop on a pane with no live terminal shows the "not
    connected" outcome SPEC.md Attachments implies (the text is the terminal component's `DETACHED_TEXT` in
    `crates/farhelm-ui/src/attachments.rs`; find the smallest way for the pane to reach it). If you add an asset file,
    run `scripts/check-desktop-assets.sh`. Browser regression on Chromium and WebKit. Gated.
11. **No partial download left behind.** In `download_verified`
    (`crates/farhelm-helm/src/provisioning/release_payloads.rs`), every early return after the `.part` file is created
    removes it with the existing `remove_if_present`. Test a mid-stream failure. Gated.
12. **Incarnations do not repeat across helm restarts.** The incarnation counter in `crates/farhelm-helm/src/manager.rs`
    starts from a random nonzero value (zero means "never connected"), drawn so that the counter keeps ample headroom
    below 2^53 to stay exact in JSON, and the doc comments that say it starts at 1 are corrected. Test the start
    function's range rather than comparing two managers. Gated.
13. **Helm-side pacing of "sessions changed".** In the host actor's refresh loop in
    `crates/farhelm-helm/src/manager.rs`, a hint-driven refresh waits until at least the supervisor's own minimum gap
    (`HINT_MIN_GAP` in `crates/farhelm-supervisor/src/service/hints.rs`; share the constant through the proto crate or
    mirror it with a comment, whichever is smaller) has passed since the previous hint-driven refresh, with at most one
    pending. Test with a flooding peer. Spec: the denial-of-service principle from the ledger, beside item 7's text.
14. **Claude refuses to derive a resume command over its own selector.** Claude's `ambiguous_derived_resume`
    (`crates/farhelm-supervisor/src/agent_kind/mod.rs`) refuses when the retained argv carries `--continue`/`-c`,
    `--resume`/`-r`, `--session-id`, or a real end-of-options `--`, with a new `SnapshotError` worded like
    `GrokAmbiguousResumeBoundary`, the same shape `plans/triage-restart-takeover-update.md` item 1 gives Codex. Read
    root `AGENTS.md` "Harness-specific code" first: this belongs in Claude's integration impl. Distinguish an option
    value that happens to equal one of those words only if Claude's own parsing makes that cheap to mirror; otherwise
    accept the same tradeoff Grok and Codex accept and log it. SPEC.md's derived-resume paragraph names Claude beside
    Grok and Codex. The TODO.md example was already added during triage. Tests: `claude --continue` and a `--` launch,
    with and without an explicit template, and a plain `claude` still deriving.
15. **OMP attribution refuses an unreadable pane runtime.** In `omp_corridor` / `is_omp_runtime_link`
    (`crates/farhelm-supervisor/src/procs/omp.rs`), for an `omp` launch, refuse when the pane process is a Bun or Node
    runtime that is not the emitter or whose arguments cannot be read. Unit test for the unreadable-pane chain. Gated
    with its ledger fallback.
16. **Snapshot must see the supervisor.** `procs::snapshot` (`crates/farhelm-supervisor/src/procs.rs`) returns an error
    when its result does not contain the supervisor's own pid, on Linux and macOS; its caller in `service/sweep.rs`
    already treats an error as "could not look". Pure test. Remove TODO.md's "Snapshot self-witness" entry. Nothing
    broader.
17. **The whole probe finishes after a dropped request.** Run `probe_host`'s work
    (`crates/farhelm-helm/src/provisioning/http.rs`, through `Provisioning::probe`) on a helm-owned task with
    `run_owned` (`crates/farhelm-helm/src/lib.rs`), extending what `plans/triage-restart-takeover-update.md` item 13 did
    for the registration step, so the backend's own process-group cleanup always runs within the probe's timeout. A
    consequence: a probe whose page went away still finishes and may register the host, as the registration step already
    does after that plan; log it as a DECISION. Regression: a dropped probe request still reaps its process group.
18. to 24. **Discards.** Remove each feedback file and its index line; no code or spec change. `docs:`. No review. PR 24
    also marks this plan's `plans/INDEX.md` line `[executed]`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-triage-confirm-ssh-identity-log.md` in the parent directory of the checkout you run in, derived as that
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
- One outcome per commit, bookmark and draft PR, in the order listed in The goal, except PR 6, which carries both ssh
  outcomes by the user's decision. Never combine other outcomes, and never split a fix spec+code outcome across PRs.
  Within this run, if a PR needs correcting, restructure it rather than stacking a correction on top.
- Commit messages and PR titles use Conventional Commits. The type reflects the user-visible effect: `fix:` for items 1,
  2, 6, 8 to 18 when they change behavior; `docs:` for items 3, 4, 5, 7, 19 to 24, and for any gated item whose gate
  tripped. Every `fix:` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); use `kind: none` with a reason where nothing changes for a user; validate
  with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing the plan stack is the user's job.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression in each PR, through
`scripts/record-test-run.py` for Rust test execution (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Typical choices, not a checklist:

- Spec-only and discard PRs (3, 4, 5, 7, 19 to 24, and tripped gates): `dprint check` on the changed files, nothing
  else.
- PRs 1 and 2 span proto, helm, supervisor and UI: `cargo fmt --all -- --check`, clippy on the touched crates, the
  supervisor delete tests and the helm's delete and replace forwarding tests through the recorder,
  `cargo check -p
  farhelm-ui` and the UI unit tests for those views, and the nearest Delete and Replace Playwright
  specs on Chromium and WebKit through the recorder when the change reaches browser behavior.
- Other Rust PRs (6, 8, 9, 10, 12, 13, 14, 15, 16, 17, 18): `cargo fmt --all -- --check`, clippy on the touched crate,
  and focused nextest selections for the touched module's tests and the new regression; PR 10 also
  `cargo check -p
  farhelm-ui`.
- JS PR (11): the new and nearest existing Playwright specs on Chromium and WebKit through the recorder, after the
  builds root `AGENTS.md` names, and `scripts/check-desktop-assets.sh` if an asset file was added.
- Any PR that changes Rust or browser tests or their helpers: run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.

### Review gate

Before finishing each PR other than a pure discard (4, 5, 19 to 24), use the active galaxy-brain skill to delegate two
independent reviews of that PR's changes. The user demands both reviewers (P1): a fresh-context agent on Opus 5.5 at
high effort, and a fresh-context agent on gpt-6-astra at high effort, shelled out to the other harness when the
executing one cannot reach that model natively. No review swarm. Each prompt carries the full charter, because the
reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

Each prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory; one per reviewer), and the acceptance criteria for that PR: its `TRIAGE_OUTCOMES.md`
entry (both entries for PR 6), its item above, and the plan-time decisions that apply to it. For a PR that changes tests
or fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. The two
reviews may run concurrently. Address what both reviewers find before moving on; where they disagree, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a new subsystem, registry, recovery protocol,
compatibility layer, a wire field other than item 1's authorized precondition, or anything else the ledger entry did not
imply; these are examples, not a blacklist), and whenever the same component has needed repeated corrective review
rounds, run a fresh-context review through galaxy-brain with this charter, supplying the ledger entry, the user
decisions above, this outline, the current diff and the proposed departure (what changed, why it is necessary, and which
simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. For items 8 to 13
and 16, a departure of that size is the complexity gate tripping, not a reason for a reassessment.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the precondition each prompt sends (items 1 and 2), the ProxyJump result (item
6), every gate that tripped and why (items 8 to 13, 16), how item 8 recognizes a post-nudge state, the `--ensure-hosts`
outcome (item 10), how item 14 shares the pacing constant, the argv words item 15 treats as selectors, item 18's
abandoned-probe registration, every disagreement between the two reviewers, and every spec wording change. The user will
ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. P3 and item 16's ledger fallback are agreed fallbacks: apply them and continue. For any other item that
needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing, step 7). Because the PRs
form one linear stack, later items sit on top of the blocked one: finish the PRs before it, record the question, and
close the plan as blocked rather than building past it. If current code or specs have moved so that a recorded decision
no longer applies, that is a question for the user too, per root `AGENTS.md` (Execute triage outcomes); never re-triage
an item yourself.

## Done criterion

The plan is complete when all 24 draft PRs exist as one linear stack on the plan stack's tip, each satisfies its ledger
entry's Completion criteria as refined by the plan-time decisions (or the complexity gate's alternative where it
tripped), each non-discard PR has passed both reviews, each has updated its own `TRIAGE_OUTCOMES.md` Execution field(s)
and removed its queue item(s) where required, and PR 24 has marked this plan's `plans/INDEX.md` line `[executed]`. Open,
not merged: merging is the user's job. Then close the plan per `plans/AGENTS.md` (Executing, step 8): write its report,
write a closing entry in its log, and stop the watchdog.
