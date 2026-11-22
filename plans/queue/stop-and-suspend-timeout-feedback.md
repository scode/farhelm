# Show Stop and suspend's 20-second reply notice and real timeout failures

Written against main at 8007a789 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Show Stop and suspend's continuing work and actual timeout failures.** The desktop drops every 504 from this
> command. A reply from the helm's 20-second bound means the operation is still running. Add a helm-only typed marker to
> that reply and show its program-neutral explanation in the session row and header, including while Stop runs before
> the host phase changes. A supervisor's own 504 has no continuation marker and must show as a failure. Cover both
> responses, and keep the other host and environment operations' reply handling separate from this fix.

In product terms: when **Stop and suspend** hits the helm's 20-second reply bound, mark that HTTP 504 reply with a
helm-only typed marker and show its honest, program-neutral explanation (stating that the operation did not finish
within 20 seconds and **may or may not still be running**, without claiming it is definitely still running) in neutral
ink without a `"Stop: "` prefix in the session row, menu, and pane header — including while Stop is still running before
the host phase changes — and clear it automatically once the host reaches `Suspended` or `Deleting` (or when another
action starts on the session). When the supervisor itself times out on Stop (an unmarked 504), surface it as a normal
danger-red failure with `"Stop: "`. Leave every other host and environment operation's reply handling unchanged.

### Where it stands (from planning research at main 8007a789; line numbers approximate)

- **Helm & proto (`crates/farhelm-proto/src/http.rs`, `ui_channel.rs`, `crates/farhelm-helm/src/sessions.rs`,
  `crates/farhelm-helm/src/sessions/sandbox.rs`):**
  - `POST /api/sessions/{id}/stop-and-suspend` (`sandbox.rs:528-592`): `stop_and_suspend_carries_on()` currently returns
    a 504 wording that says _"it carries on"_. Update it to be strictly honest per the maintainer's decision (it did not
    finish within 20 seconds and may or may not still be running; it will suspend the environment only if Stop finishes
    cleanly; check the session list for the result — never claiming it is definitely still running). Also update
    `STOP_AND_SUSPEND_CARRIES_ON` in `crates/farhelm-helm/src/tensorlake/hosts_tests.rs:2614` alongside adding
    marked-vs-unmarked 504 assertions.
  - Add a helm-only header constant in `farhelm-proto/src/http.rs` and a corresponding `HelmMarker` variant in
    `farhelm-proto/src/ui_channel.rs:214-303` (`HelmMarker::ALL`, `HelmMarker::header()`).
    `farhelm-helm/src/ui_channel.rs:960` forwards every `HelmMarker::ALL` header onto `Reply.markers` automatically.
  - The 20-second reply bound is constructed directly in `await_host_reply`
    (`crates/farhelm-helm/src/sessions.rs:3783-3812`). Add an `Option<HelmMarker>` parameter to `await_host_reply`
    (keeping `within_host_reply_bound_saying` passing `None` for all other routes) and a small
    `within_host_reply_bound_marked` helper used ONLY by `stop_and_suspend`, attaching the header strictly in the
    `Err(_elapsed)` arm. No new `thiserror` error or `lib.rs::http_error` change is needed: a supervisor
    `ErrorKind::Timeout` returned inside the 20s bound takes `Ok(Ok(response))` and can never carry the header, and
    `/api/hosts/{id}/suspend`, `/resume`, and delete routes remain untouched (`launcher/launch/wake.rs:146` relies on
    `reason.markers.is_empty()` for `/resume`).
- **Desktop GUI (`crates/gui/farhelm-gui/src/operations.rs`, `crates/gui/farhelm-gui/src/session_list/environment.rs`,
  `crates/gui/farhelm-gui/src/session_list/outcome.rs`):**
  - `environment.rs:664-696` currently calls `operations.forget` on every HTTP 504 (`CARRIES_ON`) for Suspend, Resume,
    and Stop and suspend. Pass `asked.verb` into `environment::settle` so Suspend and Resume remain completely
    unchanged; for `Work::StopAndSuspend`, check the new `HelmMarker`:
    - Marked 504: if the session's host has already reached `Suspended` or `Deleting`, call `operations.forget`
      immediately instead of recording a notice; otherwise record it as a neutral/informational notice via a minimal
      tone addition on `Operations::failures` (`Tone::Danger` default on `settle`/`note`, plus `Tone::Notice` via
      `Operations::notice`), reusing `Operations`'s automatic clear on any next session action, hold, or reset.
    - Also observe pushed store updates via `session::observe_current` (mirroring
      `session_pane/card.rs::forget_settled`) to call `Operations::forget(&OpKey::session(key), cx)` once the host
      reaches `Suspended` or `Deleting` — actually forgetting it rather than only hiding it at render time, so a later
      Resume from the Hosts panel (`OpKey::host`) never makes an old Stop-and-suspend notice reappear.
    - Unmarked 504 (supervisor timeout): keep as a normal danger-red failure with `"Stop: "` prefix.
  - All outcome rendering (`row.rs`, `menu.rs`, `session_pane/header.rs`, `session_pane/card.rs`) delegates to
    `crates/gui/farhelm-gui/src/session_list/outcome.rs` (`row_line`, `menu_line`, `pane_line`). Update `outcome.rs` so
    `Tone::Notice` renders in neutral ink (`palette.fg_1`, or `palette.on_fill` on a selected row) without `"Stop: "`,
    matching existing outcome lines' ARIA role behavior (do NOT add `Role::Status` unconditionally, which would break
    `wait_line`'s single-copy announcement rule at `outcome.rs:242-244`), while `Tone::Danger` keeps `palette.danger`
    and `"Stop: "`.
- **Specs:** `SPEC.md:1705-1710` and `SPEC_impl.md:266-277, 6841` currently state the old _"carries on"_ wording and
  **must** be updated to match the honest wording and helm-only continuation marker.

### Acceptance criteria

1. **Helm-only typed marker on Stop and suspend's 20-second bound only:**
   - A 504 from `stop_and_suspend`'s 20-second helm bound carries the new `HelmMarker` and an honest, program-neutral
     message that does NOT claim the operation is definitely still running (it states it did not finish within 20
     seconds, may or may not still be running, and suspends the environment only if Stop finishes cleanly).
   - A supervisor 504 (`ErrorKind::Timeout`) from Stop carries NO continuation marker.
   - Suspend, Resume, EnvironmentDelete, and all other routes do NOT emit the new marker and keep their existing reply
     handling untouched.
2. **Desktop GUI presentation and lifecycle:**
   - Marked 504 on `Work::StopAndSuspend` renders in neutral/muted ink (`palette.fg_1`, or `palette.on_fill` when the
     session row is selected) with NO `"Stop: "` prefix in the session row, menu, and session pane header (and card),
     including while Stop runs before the host phase changes.
   - The notice is forgotten (`Operations::forget`) once the host reaches `Suspended` or `Deleting` (both on arrival if
     already there and on pushed store updates, so a subsequent Resume from Hosts never resurrects it), or when another
     operation starts on that session.
   - Unmarked 504 on `Work::StopAndSuspend` renders as a normal failure in `palette.danger` with `"Stop: "`.
3. **Specs and tests:**
   - `SPEC.md:1705-1710` and `SPEC_impl.md:266-277, 6841` are updated to reflect the honest wording and marker.
   - Unit/integration tests cover `farhelm-proto` marker parsing/forwarding, `farhelm-helm` marked 20s timeout vs
     unmarked supervisor 504 (updating `STOP_AND_SUSPEND_CARRIES_ON` in `hosts_tests.rs`), and `farhelm-gui`
     (`environment_tests.rs`, `header_tests.rs`, `session_pane::environment_tests`, `operations_tests`) covering neutral
     marked notice before phase change, auto-clear on `Suspended`/`Deleting` without reappearing after Resume, and
     danger failure on unmarked 504.
   - Remove the TODO.md entry in the last PR.

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- Render the 20-second Stop and suspend notice in neutral/muted ink without a `"Stop: "` prefix, while real failures
  (including unmarked supervisor 504s) stay danger red with `"Stop: "`.
- **Wording constraint:** Do NOT claim the operation is definitely still running — state honestly that it may or may not
  still be running.
- Clear the notice automatically once the host reaches `Suspended` (or `Deleting`), or when another action starts on
  that session.
- Keep all other host and environment operations' reply handling separate and untouched.
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: `No release note needed.` (no highlight; folded full list only).

**Binding repository constraints:**

- `SPEC.md:1705-1710` and `SPEC_impl.md:266-277, 6841`.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and "Instructions by path".

**Planner proposals:**

- Attach the `HelmMarker` directly in `await_host_reply` (`sessions.rs`) via a small `within_host_reply_bound_marked`
  wrapper rather than adding a new error type through `lib.rs::http_error`.
- Reuse `Operations::failures` with minimal `Tone::Danger`/`Tone::Notice` metadata (plus `Operations::notice`) rather
  than adding a parallel notice map, so automatic clearing on any next session action stays unified.
- Slice into 2 bite-sized PRs: (1) proto marker + helm Stop and suspend 20s-bound marker, honest wording, spec updates,
  and proto/helm tests, (2) desktop GUI neutral notice + auto-forget + unmarked 504 failure + GUI tests + TODO removal.

## Outline

1. `plan/stop-and-suspend-timeout-feedback/01-helm-continuation-marker`: add the Stop and suspend 20s-bound `HelmMarker`
   in `farhelm-proto` (`http.rs`, `ui_channel.rs`), attach it strictly in `await_host_reply`'s timeout arm via
   `within_host_reply_bound_marked` in `crates/farhelm-helm/src/sessions.rs` + `sessions/sandbox.rs` with honest
   program-neutral wording ("may or may not still be running"), update `SPEC.md` and `SPEC_impl.md`, and update/add
   proto + helm tests (`ui_channel_tests`, `hosts_tests.rs`) verifying marked 20s bound vs unmarked supervisor 504 while
   leaving other routes unchanged. `fix: mark Stop and suspend 20s timeout replies in helm` (with `kind: fixed`
   changelog fragment).
2. `plan/stop-and-suspend-timeout-feedback/02-gui-notice-and-failure`: distinguish marked 504 notice (neutral ink, no
   `"Stop: "` prefix, forgotten via `Operations::forget` on `Suspended`/`Deleting` arrival, pushed store update, or next
   action) from unmarked 504 failure (danger red, `"Stop: "`) for `Work::StopAndSuspend` in `farhelm-gui`
   (`operations.rs`, `session_list/environment.rs`, `session_list/outcome.rs`), add GUI tests, and remove the TODO.md
   entry ("Show Stop and suspend's continuing work and actual timeout failures").
   `fix: show Stop and suspend timeout notices in desktop app` (amends PR 1's changelog fragment).

## Fallbacks

- If existing GUI tests assert `"Stop: "` on non-504 Stop and suspend failures (such as 409 in
  `environment_tests.rs:777`), preserve `"Stop: "` for all failure outcomes; only the marked 504 neutral notice omits
  the prefix.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-stop-and-suspend-timeout-feedback-log.md` in the parent directory of the checkout you run in, derived as
that section says, and the watchdog's status file is `farhelm-plan-stop-and-suspend-timeout-feedback-watchdog.json`
beside it. Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the
agent scratch directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run targeted tests in `farhelm-proto` (`ui_channel`), `farhelm-helm` (`ui_channel_tests`,
`tensorlake::hosts_tests`), and `farhelm-gui` (`session_list::environment_tests`, `session_pane::header_tests`,
`session_pane::environment_tests`, `operations_tests`), `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `dprint check` on changed Markdown, and
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
- The last PR of the stack removes the TODO.md entry this plan covers ("Show Stop and suspend's continuing work and
  actual timeout failures"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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
