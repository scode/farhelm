# Execute the 2026-10-01 triage outcomes: sign-in, status reading, Pi, slow hosts, paths, transfers

Written against main at d056dc9 on 2026-10-01. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Carry out the 17 triage outcomes recorded in root `TRIAGE_OUTCOMES.md` whose Execution field reads
`` planned in `plans/triage-signin-status-paths.md` ``, exactly as root `AGENTS.md` section "Execute triage outcomes"
prescribes: one reviewable commit, one stable bookmark and one draft PR per outcome, in a single linear stack. The
outcomes, in ledger order, which is also the stack order:

1. `desktop-reauth-remount-loses-action.md` (fix spec+code)
2. `desktop-reauth-failure-dead-end.md` (fix code)
3. `seen-toggle-report-panics-after-unmount.md` (fix code)
4. `hosts-panel-leaks-page-lock.md` (fix code)
5. `profile-popup-leaks-page-lock.md` (fix code)
6. `codex-last-option-reads-idle.md` (fix spec+code)
7. `codex-working-backstop-never-matches.md` (fix code)
8. `claude-last-option-reads-idle.md` (fix code)
9. `claude-spinner-window-too-short.md` (discard)
10. `claude-spinner-rejects-multiword.md` (discard)
11. `pi-reporter-asset-not-renamed.md` (fix code)
12. `checkout-preview-blocks-read-loop.md` (fix spec)
13. `repo-search-blocking-scan.md` (fix spec)
14. `delete-holds-attachments-lock-through-archive.md` (fix spec)
15. `restart-cwd-lossy-non-utf8.md` (fix spec+code)
16. `non-utf8-farhelm-path-breaks-launch.md` (fix code)
17. `sftp-overall-deadline-fails-slow-links.md` (fix spec)

Each outcome's `TRIAGE_OUTCOMES.md` entry (Assessment, Decision, Completion criteria) is the authoritative statement of
what that PR must do. Read the entry and its feedback file under `review_feedback_queue/` before starting the item. This
file adds the plan-time decisions and the mechanisms chosen to satisfy those entries; where the two seem to disagree,
the ledger entry wins and the disagreement is a DECISION to log (or a question, per Unattended fallback).

Acceptance criteria:

- 17 draft PRs exist, one per outcome, stacked in the order above on top of the plan stack's tip (see PR discipline).
- Each PR does what its ledger entry's Completion criteria say, removes its feedback file and its line in
  `review_feedback_queue/INDEX.md`, and updates its own `TRIAGE_OUTCOMES.md` Execution field to `complete` with the jj
  change ID, bookmark and PR URL (record the change ID and bookmark before creating the PR, then add the URL to the same
  change and push again; no separate bookkeeping PR).
- Every PR except 9 and 10 (and 13, if it ends up carrying no spec text; see its item) has passed the review gate.
- PR 17 also changes this plan's line in `plans/INDEX.md` to `[executed]`. Do not delete this plan file.
- No PR is marked ready, and nothing is merged.

## Requirement sources

Keep these apart; a reviewer should be able to tell which is which.

**The user's request:** "use the planning system to queue up execution of the code review items we just triaged".

**The user's triage decisions (2026-10-01),** recorded in full in each ledger entry's Decision field. In short:

- Sign-in: the desktop app is the primary surface. Signing in again after a token rotation may reset the page and lose
  open forms, dialogs and drafts; browser sign-in friction is acceptable; no significant complexity is spent preserving
  UI state. Still required: an action the user started is never lost silently, sign-in recovery never crashes or leaves
  the window dead, and a failed desktop re-sign-in can be retried. Goes into SPEC.md with item 1.
- Claude Code and Codex are the first-class harnesses; a clear, definite gap in their activity detection is fixed.
  Activity and session tracking for other harnesses are intentionally partial, and gaps there are expected and not worth
  raising in review. Goes into SPEC.md with item 6.
- A slow host is slow: no complexity is spent compensating for a slow remote host or a slow helm machine, filesystem or
  otherwise, as long as the helm stays usable. Within a host, sessions stay independent: terminal I/O must not block on
  a large operation such as a git clone. Goes into SPEC.md with item 12; item 14 clarifies "Waiting between operations
  on one host" accordingly.
- Paths that are not valid UTF-8 are not supported: refused clearly at every surface, never silently corrupted. Goes
  into SPEC.md with item 15. Item 16 sweeps the remaining lossy conversions under a complexity gate: change a site only
  when the fix is simple and clearly right; anything non-trivial needs the user's judgment.
- Payload transfers during host add/install and update have no fixed overall timeout, only stall timeouts. Goes into
  SPEC.md with item 17.
- The two page-lock leaks (items 4, 5) are fixed because they are easy; no significant complexity.
- The Pi asset (item 11) is renamed and a test added; no migration or cleanup for hosts already carrying the stale file.

**The user's plan-time decisions (2026-10-01):**

- P1. Review gate: a fresh-context Opus 5.5 agent at high effort with the general review charter (see Review gate). No
  review swarm.
- P2. No review for the trivial discard PRs (9, 10), which only remove a queue item. Spec PRs and code PRs are reviewed.
- P3. Item 16's sweep runs unattended: fix only sites that are simple and clearly right, leave every other site
  unchanged, and list the left-alone sites for the user in the final report and in that PR's description. Do not block
  on them.
- P4. Do not run `scripts/capture-agent-screens.py`. Item 8's test uses a screen edited from the real captured Claude
  question screen, with the highlight moved to the last option.
- P5. If the desktop build's system packages (webkit2gtk/gtk development packages; see the CI workflow's apt list) are
  missing on the executing machine, say so in the affected PR's description and run the browser-side checks instead of
  blocking. State plainly which desktop-only code went unexecuted.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes;
Releases and the changelog; Agent screen fixtures; Harness-specific code; Sharing the machine with other agents; Agent
scratch space), `plans/AGENTS.md` (Executing), `review_feedback_queue/AGENTS.md` (Lifecycle), and
`docs/agent-screen-fixtures.md`.

**Planner proposals** are the per-item mechanisms below, marked as such. A fresh-context planning review checked them
for unnecessary scope; its tightenings are folded in.

## Per-item outline

Planner proposals unless they restate a ledger entry. Line numbers are approximate; find the code by name.

1. **Desktop re-sign-in loses the triggering action.** Write the sign-in principle into SPEC.md (and SPEC_impl.md if the
   mechanism needs recording). The bug: after a native 401, `retry_desktop_request` (`crates/farhelm-ui/src/api.rs`)
   bumps the desktop auth generation before the retried request is sent and read, and the bootstrap gate
   (`crates/farhelm-ui/src/auth.rs`) stops rendering `AppBody`, which cancels the UI task awaiting the retry. The
   request functions have dozens of call sites that read the body inside that cancellable task, so threading a
   "replaced" flag through every caller is ruled out as significant complexity. Choose between two contained mechanisms,
   in this order of preference, and log the choice as a DECISION:
   - (a) Keep `AppBody` mounted while the desktop webview re-authenticates: change only the gate in `auth.rs` so a
     re-authentication after the first successful one runs without unmounting the app. Adopt this only after checking
     that `assets/desktop-auth.js`, the webview's event socket and anything else running in the page tolerate the
     re-authentication happening underneath a live page. It also removes the desktop trigger behind items 3 to 5, which
     still get their own fixes.
   - (b) In the retry path, read the retried response body before triggering the webview re-authentication, then hand
     the caller a rebuilt response. Under (b) the remount still discards the caller's UI afterwards, so the page that
     comes back must show the action's effect (a re-read listing does that for success), and an error from the retried
     request must not vanish silently: surface it in a way that survives the remount, using an existing facility if one
     exists. If (b) cannot meet "never lost silently" without new machinery, stop and treat it as a question (Unattended
     fallback). Add regression coverage at the existing test seam for `retry_desktop_request`.
2. **Desktop re-sign-in failure is a dead end.** Retry only, no automatic backoff and no retryable/terminal
   classification: show a Retry control on every failure in the gate's failure branch, wired to the existing re-auth
   trigger (bumping the generation already clears the failure and restarts authentication) or to the authentication
   future's restart, whichever is simpler.
3. **Read/unread toggle can crash after unmount.** In the manual toggle's report (`crates/farhelm-ui/src/list/view.rs`,
   around the `errors.write()` in the seen report), use `try_write()` and drop the update on failure. Document on
   `SeenWriteReport` that a report can run after its caller unmounted.
4. **Hosts panel strands the page lock.** Replace the bare `ops.claim()` in the hosts panel's shared request runner
   (`crates/farhelm-ui/src/hosts.rs`) with `claim_guard()` and move the guard into the spawned task.
5. **Profile popup strands the page lock.** Same in both handlers in `crates/farhelm-ui/src/profiles.rs`; in the save
   handler, take the guard after the local early-return validation, or let those returns drop it.
6. **Codex dialog with the last option highlighted reads Idle.** Write the first-class-harness principle into SPEC.md
   (the review-only filter in `review_feedback_queue/FILTER.md` does not substitute for it). In
   `codex_dialog_may_be_open` (`crates/farhelm-supervisor/src/agent_kind/screen_reader.rs`), also accept a numbered
   option directly above the `›` row as menu evidence (or require every row from `›` down to be an option or a known
   footer, whichever is smaller and passes every existing fixture). Add the fixture pair
   `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/waiting-derived-last-option.{txt,title}`, edited from
   `waiting-trust.{txt,title}` with the `›` moved to option 2. Edited screens are named `*-derived-*` per
   `docs/agent-screen-fixtures.md`; never save one under a real-capture name. The existing fixture-driven test checks
   every fixture against the state in its file name, so no new test function should be needed.
7. **Codex "working" backstop never matches.** `codex_composer_bounds`
   (`crates/farhelm-supervisor/src/agent_kind/mod.rs`) requires the second-to-last screen line to be padding, but real
   0.159.0 screens end with two footer rows. Locate the composer by its `›` prompt row followed only by blank or
   indented rows down to the bottom, and allow the blank rows the real fixtures show between the status line and the
   prompt. Test by running the existing `working-*` fixtures through the reader with an empty title; a short loop over
   them is enough.
8. **Claude question with the last option highlighted reads Idle.** Fix `claude_input_box_rule` (`screen_reader.rs`)
   with exactly one of the ledger's two options, requiring the input box's full shape (rule, `❯` row, closing rule) or
   checking the dialog footer before accepting a box, whichever passes every existing fixture with the smaller change.
   Correct the comment that says menus never draw `❯` under a rule. Add
   `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/waiting-derived-last-option.{txt,title}`, edited
   from `waiting-question.{txt,title}` with `❯` on option 4 at column 0 and the other options' indentation as captured
   (P4).
9. and 10. **Discards.** Remove the feedback file and its index line, and update the ledger entry. No code, spec or
   changelog change, and no review gate (P2).
10. **Pi plugin changed without a new file name.** Publish the current Pi asset under a new name
    (`farhelm-conversation-v2.ts` in `integrations/pi/`; OMP's file of the same name lives in `integrations/omp/`, so
    there is no collision), updating `crates/farhelm-supervisor/src/pi_extension.rs` and any spec or doc text naming the
    Pi file. Add a test that pins a table of (integration directory, published file name, sha256 of the bytes) for both
    the Pi and OMP assets, so changing an asset's bytes without renaming it fails; `sha2` is already a supervisor
    dependency. Do not switch to content-derived file names: that would change publication paths and the OMP launch
    record. No migration or cleanup of the old file.
11. **Slow-host principle.** Write the principle into SPEC.md beside "Healthy local filesystems" and "Waiting between
    operations on one host", keeping the requirement that one session's long operation never blocks another session's
    terminal I/O. Spec only.
12. **Repository search on a slow folder.** Confirm item 12's text covers this finding; if it does not, add the missing
    sentence here. If no spec text is needed, this PR is a queue removal only and, like 9 and 10, skips the review gate;
    log that as a DECISION.
13. **Delete pauses typing on the host.** Clarify "Waiting between operations on one host" so terminal I/O may wait on
    brief, bounded local work (such as Delete's renames, fsyncs and final database commit under the terminal lock), but
    never on long operations or kill grace periods. Spec only.
14. **Restart starts the agent in the home directory.** Write the non-UTF-8 principle into SPEC.md. Use strict
    conversion (`to_str()`) for the canonical working directory at create and in the restart/retry identity check
    (`ensure_cwd_identity` and its create-side counterpart in `crates/farhelm-supervisor/src/service/core.rs`), refusing
    with a clear message. Add a test with a symlink to a non-UTF-8 directory.
15. **Non-UTF-8 farhelm path or state directory breaks every launch.** Refuse a non-UTF-8 farhelm binary path or state
    directory at supervisor startup with a message naming the path; correct the field docs and startup warning that
    claim only degraded capture; add a test for the startup refusal. Then the sweep (P3): limit it to non-test
    conversions of paths whose lossy text reaches a command line, saved state or the wire; lossy text in logs and on
    screen is fine. There are roughly 150 `to_string_lossy` sites across the crates, many in tests or on non-path
    strings. In the PR description and the final report, list individually every site changed and every site left for
    the user's judgment, and summarize the left-alone sites by category with one reason each.
16. **Stall-only transfer timeouts.** Write into SPEC.md that payload transfers during host add/install and update have
    no fixed overall timeout and time out only on stalls. Main already does this for uploads (#888, #1143); check both
    paths anyway. If a fixed overall deadline is found, fix it in this PR when the change is trivial; otherwise treat it
    as a question. This PR also marks this plan's `plans/INDEX.md` line `[executed]`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-triage-signin-status-paths-log.md` in the parent directory of the checkout you run in, derived as that
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
- Commit messages and PR titles use Conventional Commits. The type reflects the user-visible effect: `fix:` for items
  1-8, 11, 15 and 16; `docs:` for the spec-only and discard items (9, 10, 12, 13, 14, 17), unless item 17 ends up
  changing code. Every `fix:` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing the plan stack is the user's job.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression in each PR, through
`scripts/record-test-run.py` for Rust test execution (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Typical choices, not a checklist:

- Docs-only PRs: `dprint check` on the changed files, nothing else.
- Supervisor PRs (6-8, 11, 15, 16): `cargo fmt --all -- --check`, clippy on the touched crate, and focused nextest
  selections for the touched modules (the screen-fixture tests, `pi_extension`, the restart identity check, launch and
  startup).
- UI PRs (1-5): `cargo check -p farhelm-ui` and the desktop feature check
  `cargo check -p farhelm-ui --features
  desktop`. A regression test for desktop-only code (items 1 and 2, possibly 3)
  needs the recorded `cargo nextest run -p farhelm-ui --features desktop` selection. If the desktop system packages are
  missing, P5 applies: say so in the PR and state which desktop-only code went unexecuted. Run the browser JS harness
  only if a change touches asset JS.
- Any PR that changes Rust or browser tests or their fixtures: run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.

### Review gate

Before finishing each PR other than 9 and 10 (and 13 when it has no spec text), use the active galaxy-brain skill to
delegate a review of that PR's changes. The user demands this reviewer: a fresh-context agent on Opus 5.5 at high
effort, with no review swarm (P1). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria for that PR: its `TRIAGE_OUTCOMES.md` entry and its item above.
For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Address what the reviewer finds before moving on. Do not write a launch command here or in the
log; the harness-shellout skill owns launch mechanics.

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
alternatives considered: in particular item 1's mechanism, item 6's and item 8's chosen option, item 13's review skip,
and every judgment call in item 16's sweep. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback (P3 and P5 are the agreed ones) or the user's decision; a review
finding or a log entry is not authorization. If an item needs such a decision, record the concrete tradeoff, and block
per `plans/AGENTS.md` (Executing, step 7). Because the PRs form one linear stack, later items sit on top of the blocked
one: finish the PRs before it, record the question, and close the plan as blocked rather than building past it. If
current code or specs have moved so that a recorded decision no longer applies, that is a question for the user too, per
root `AGENTS.md` (Execute triage outcomes); never re-triage an item yourself.

## Done criterion

The plan is complete when all 17 draft PRs exist as one linear stack on the plan stack's tip, each satisfies its ledger
entry's Completion criteria, each that requires it has passed the review gate, each has updated its own
`TRIAGE_OUTCOMES.md` Execution field and removed its queue item, and PR 17 has marked this plan's `plans/INDEX.md` line
`[executed]`. Open, not merged: merging is the user's job. Then close the plan per `plans/AGENTS.md` (Executing, step
8): write its report, write a closing entry in the log, and stop the watchdog.
