# Sweep out browser sign-in and failed-save preservation

Written against main at b9619c88 on 2026-10-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file says which of their sentences to change.

This plan has no dependency on another plan. NOTE: `gui-saves-last-write-wins.md` landed on 2026-10-22 (#2562, #2583),
after this file's code references were taken. It changed the same shared-preferences writer in
`crates/farhelm-ui/src/api.rs` that this plan sweeps: a failed number-key save is now reported and discarded, through a
per-field switch, `PreferenceField::discards_failed_writes`, that is true only for number keys, and the number-key half
of the template-deletion retirement is gone. Once every failed save is discarded, that switch has nothing left to
decide; remove it rather than turning it on for every field. Every code location below predates #2583: re-find each one
on current main, and expect the number-key parts to be gone already.

## The goal

SPEC.md now says two things it did not say before 2026-10-22, and the code still carries machinery built for the
opposite. This plan removes that machinery, so the code is smaller and matches the spec:

- When a browser GUI's sign-in expires, the page is treated as reloaded from scratch. Nothing the helm had not accepted
  is kept, replayed, restored or merged back after signing in, and recovery is a literal page reload ("Signing in
  again").
- A GUI save that fails, for any reason, in the browser or the desktop app, is reported and then discarded. The GUI does
  not keep the change, does not treat it as the current state, and does not retry or replay it; the next view or save
  starts from what the helm reports (the "Decided 2026-10-22" paragraph under "One GUI at a time").

TODO.md entry this plan covers (Near term), verbatim: "**Sweep out browser sign-in and failed-save preservation.**
Decided 2026-10-22 (SPEC.md, "Signing in again" and the failed-save rule under "One GUI at a time"): an expired browser
sign-in is treated as reloading the interface from scratch, recovered by a literal page reload or as close to one as
possible, and a failed GUI save in the browser or the desktop app is reported and discarded, never kept, treated as
current or replayed. The spec forbids adding new machinery of either kind; this entry removes what already exists. Known
places to start: the shared-preferences writer in `crates/farhelm-ui/src/api.rs`, which keeps failed writes as an
overlay on later reads and replays them after reauthentication (the "reauthentication seed", failed link choices and
contact clears that "survive reauthentication"); the browser token exchange in `crates/farhelm-ui/src/auth.rs`, which
deliberately keeps the app mounted and remounts only the active surface and feed instead of reloading the page
(`reauthentication_keeps_the_app_mounted`); and anything that retries or replays a save or click after sign-in. Search
the UI for "reauthentication", "replay", "overlay" and failed-save retention to find the rest. Each removal should make
the code smaller; the desktop app's own authentication path is out of scope, since it never signs in again."

One correction to that entry, verified by the planning session: `reauthentication_keeps_the_app_mounted`
(`crates/farhelm-ui/src/auth.rs`, around 604-625) tests the desktop sign-in gate (`GateState`, `cfg(native_desktop)`),
not the browser. It is the desktop app's own authentication path and stays.

### Acceptance criteria

**Browser sign-in recovery.**

- When the browser's credential is refused, the token form is shown as it is today. Once a pasted token is exchanged and
  the new device secret is stored, the page reloads (a real `location.reload()` or the nearest equivalent the renderer
  offers), instead of clearing the token-required flag and remounting the app in place. Reload after the exchange, never
  at the refusal itself: a reload at the refusal would come back to the same refused credential.
- Code that exists only so the browser app survives being remounted behind the token form, within one page load, is
  removed. The planning session's inventory found these candidates: the welcome decision kept across a remount
  (`WelcomeDecided` in `crates/farhelm-ui/src/welcome.rs`, provided in `lib.rs` around 1691, and the token-required
  check in the preferences gate around 2077), and the list's `row_ops` / `deleting` reset hooks (`list/view.rs` around
  789-811). None of them serve the desktop app: the planning review found that the desktop `AppBody` cannot remount
  after its first successful mount (Retry appears only on the failure page, and a sign-in that succeeded cannot fail
  afterwards), and these are all created inside `AppBody`. Comments that name the desktop path next to the browser's
  (`list/view.rs` around 5230) are corrected rather than taken as a reason to keep code.
- One other in-place remount remains: if the build-mismatch notice appears while the token form is showing, the app
  remounts without a reload. That notice tells the user to reload, so removing the reset hooks is still right; log it as
  a DECISION.
- Code that copes with the app being unmounted while the token form is showing (the claim guards in `hosts.rs` around
  1583-1589 and `session_view.rs` around 636, `seen_toggle_report` using `try_write`) stays: the form still replaces the
  app before the reload.

**Failed preference saves are discarded.**

- In the shared-preferences write queue (`PreferenceWrites` in `api.rs`), a write that fails, and a writer that is lost
  mid-request, forget the value instead of keeping it dirty. Serialization, latest-wins ordering, and completion
  reporting to a waiter stay. After this, "dirty" means only "a write is still pending", and laying a pending write over
  a fresh read (`template_bindings_after_read`, the trust-hint fetch overlays) is legitimate and stays. Forgetting the
  value on every settled write, success included, which retires the `acked` field, is a fine smaller shape.
- The resend after sign-in goes: `seed_with_local_changes` and its call in the preferences gate, `begin_replay`, the
  arms of `PreferenceValue::overlay` nothing uses any more (inlining the two that remain instead of keeping a cut-down
  match is fine), and the template-deletion retirement that exists only so a resend cannot bring back a deleted
  template's mark (`retire_template`, `retire_template_choice`). Keep the part of template deletion that waits for an
  in-flight write and refuses when the writer was lost. Careful with `PreferenceCompletion.retire_template`: besides
  naming what to retire, it is what lets a delete go ahead after the binding write it waited on fails (`finished` sends
  `deleting || success`). Reduce it to a plain flag rather than removing it, or the delete would refuse with "a shortcut
  write was interrupted".
- Renaming a template whose quick-launch mark or number key fails to move reports the failure today but has already put
  the new binding into this window's preferences (`list/templates.rs` around 1662-1701 writes the signal before the save
  is confirmed). The handler has already read the bindings from the helm just before; show the new binding only after
  the save succeeds, or put back the value that read returned on failure. No second read.
- The best-effort preferences in SPEC.md's "Errors and diagnostics" (list order, last selection, compact layout, the
  host setup and removal confirmation choices, turning the session-confirmation opt-out on, the feedback contact) keep
  their current behavior on screen: the choice stays in effect until the next read from the helm, and a failure is only
  logged. They are no longer kept or resent.
- Turning the session-confirmation opt-out off keeps its safety rule (SPEC.md, "Errors and diagnostics"): on a failed
  save, confirmations stay on in this client and Settings shows the error. That is not failed-save retention.

**Spec text.** SPEC.md's best-effort exception in "Errors and diagnostics" gains one clause saying a failed best-effort
save is not resent (the choice stays in effect only until the next read from the helm), worded as the exception to the
2026-10-22 failed-save rule under "One GUI at a time" that it is, so a later reader does not see the two as a conflict.
Do not restate the two 2026-10-22 rules anywhere else. SPEC_impl.md passages describing the old overlay, resend and
remount behavior are rewritten to describe what remains; the inventory found them around lines 190-191, 231-233, 238,
246-249, 410-411, 1123, 1165, 1173-1177, 2298-2299 and 2343.

**Comments.** Doc comments and inline comments that describe the old flow are rewritten or removed. The inventory found
them in `api.rs`, `auth.rs`, `lib.rs`, `activity.rs`, `sounds.rs`, `save_order.rs`, `workspace_trust.rs` and
`list/templates.rs`; search for "reauthentication", "replay", "survive" and "overlay" to find them. Those words miss
some: the module doc of `welcome.rs` (lines 6-7) says the preference queue "keeps the local choice even when its
best-effort write fails".

**Tests.** Tests that assert the old behavior are deleted or inverted to assert the value is discarded; tests that mix a
still-valid part with a resend part keep the valid part. The inventory's list is in the Outline. In the browser suite,
the case in `e2e/tests/welcome.spec.ts` asserting the welcome suppression survives sign-in recovery in the same page
goes, and the case in `e2e/tests/auth.spec.ts` asserting the same session view and socket survive sign-in is rewritten
to assert the page reloads after the exchange (its rotation and socket-teardown part stays). Other welcome cases stay
and must still pass, in particular "welcome waits for initial sign-in before deciding" (a refusal at startup, now
followed by a reload) and "a refused seen write still dismisses the welcome" (the choice stays on screen until the next
read).

**Bookkeeping.** `fix` PRs carry a changelog fragment per root `AGENTS.md`, "Changelog fragments" (a later PR extending
an earlier one's unreleased change edits that fragment instead of adding another). The last code PR removes the TODO.md
entry.

**Size.** The point of this plan is smaller code. The combined diff must remove more production code than it adds. An
addition beyond about 50 lines of production code (tests and comments not counted) is a departure that needs the scope
reassessment below.

### Out of scope, on purpose

- The desktop app's own authentication path (its gate, `GateState`, its Retry, and the idempotent installs that survive
  an `AppBody` remount such as `lib.rs` around 1495, `tooltip.js` and `sidebar-width.js`). It never signs in again in
  ordinary operation. Comment tweaks there are fine; behavior changes are not.
- The helm. The planning session found nothing there that exists only for browser re-sign-in.
- Dialogs that stay open after a refused save, keeping what the user typed next to the error (template editor, rename,
  host form, ntfy, feedback, checkout folder). That is reporting, not keeping the change: nothing survives the dialog
  closing and nothing is resent. Leave them alone and do not add spec text about it.
- Create and Replace keeping a request's identity after an unknown outcome, so that pressing it again is recognized as a
  repeat instead of launching twice. That is duplicate protection, not a replay, and nothing retries it automatically.
  It stays.
- Fire-and-forget preference saves that are not on the spec's best-effort list (the welcome screen being seen, the
  trust-hint dismissals, the terminal link-click choice, frame colors, naming sessions on quick launch, the quick-launch
  template mark) fail silently today, which already conflicts with "Every failed operation surfaces a concrete,
  actionable error". This plan discards them like the rest and does not add reporting. Name the gap in the report under
  possible follow-ups. Number-key failures are reported by `gui-saves-last-write-wins.md`.

## Requirement sources

**The user's decisions (2026-10-22), from the planning page.** Every pick was the planner's recommendation, with no
comments:

- Best-effort preferences whose save fails: "Stop keeping and resending them; the choice stays on screen until the next
  read from the helm, and the failure is still only logged. Add one clause to the best-effort exception saying a failed
  value is never resent." Chosen over the same behavior with no spec change, and over reverting the screen and reporting
  the failure.
- Other silent preference failures: "Out of scope: discard them like the rest, leave the reporting gap, and list it in
  the report as a follow-up." Chosen over adding them to the spec's silent list, and over making each report its
  failure.
- Browser recovery: "Keep the token form as the first thing shown; once the new token is accepted and stored, reload the
  page." Chosen over reloading at the refusal with a loop guard.
- Dialogs that stay open after a refused save: "Leave them alone: that is reporting the failure, not keeping the
  change."
- Duplicate protection on Create and Replace: "Keep it: it is protection against duplicates, not replay."
- PR shape: as proposed in the Outline, and the executor may split further where a PR gets large.
- Review gate: a single fresh-context Opus 5.5 agent at high effort with the general charter. No-workhorse mode, which
  `plans/.agents/planning.md` requires.

The maintainer's words behind the 2026-10-22 rules, from `gui-saves-last-write-wins.md`'s Decisions: "We spend NO
complexity trying to make browser based login expiry be fine grained, or save state associated with one click that
happened _just_ as the login occurred, etc." and "The goal is that the goal is left in the simplest state even if it
means abandoning all work that's already been done."

**Binding repository constraints:** SPEC.md, "Signing in again", "One GUI at a time" and "Errors and diagnostics"; root
`AGENTS.md`, "Changelog fragments", "Conventional Commits", "Code rules" and "Tests and validation", with
`.agents/validation.md`; `.agents/test-authoring.md` for any test change.

**Planner proposals (checked by a fresh-context planning review):** the PR order and the outline below, the inventory's
line references, and the size rule above. A planner proposal is not a requirement: a smaller design that meets the same
acceptance criteria is fine, logged as a DECISION.

## Outline

The planning session's read-only inventory of main at b9619c88 estimated about 700 lines removable (roughly 200 of
production code and comments, 300 of Rust tests, 110 of browser tests, 15 of SPEC_impl.md) against 10-15 lines added.
Treat those numbers as a rough guide, not a target.

The proposed stack, bottom up:

1. `docs`: the best-effort clause in SPEC.md's "Errors and diagnostics". No fragment.
2. `fix`: browser recovery reloads the page. Replace the token-required flag clear in `TokenPrompt` (`auth.rs` around
   469-474) with a reload after the device secret is stored. The `web-sys` `Location` feature is already enabled and
   `main.rs` (around 52) already calls `location()`, so `web_sys::window()` `location().reload()` needs nothing new.
   Remove what only served the in-place remount (see Acceptance criteria), rewrite the comments that describe it, and
   change the two browser specs. Changelog fragment.
3. `fix`: failed preference saves are discarded. Change `finished` and `writer_lost` to forget the value; remove
   `seed_with_local_changes` and its call (`lib.rs` around 2074, with the gate's doc around 2016-2023), `begin_replay`,
   the dead `overlay` arms, and the template-deletion retirement (keeping the delete-may-proceed flag); stop the rename
   handler showing a binding before its save succeeds (`list/templates.rs` around 1662-1701); rewrite comments and
   SPEC_impl.md; update the tests. Changelog fragment, or an edit of PR 2's if the executor judges one entry covers
   both. Removes the TODO.md entry.

PR 3 may be split if it gets large (for example, the rename fix on its own). Avoid churn: do not add code in one PR that
a later one deletes.

The Rust tests the inventory listed, all in `api.rs` (`preference_write_tests` and nearby):

- Delete: `welcome_seen_failure_keeps_replay_intent`, `number_binding_delete_retires_only_the_deleted_name`,
  `failed_feedback_contact_clear_overlays_and_replays_as_null`,
  `a_failed_write_stays_dirty_and_is_claimed_exactly_once_for_replay`,
  `an_acknowledged_field_is_not_overlaid_onto_a_fetched_seed`.
- Invert to assert the value is discarded: `acknowledgment_failure_and_cancellation_preserve_dirty_recovery`,
  `a_lost_writer_releases_the_field_without_forgetting_the_value`.
- Trim the resend half: `failed_and_cancelled_shortcut_writes_release_reads`,
  `agent_trust_dismissals_keep_independent_dirty_intent`, `rename_mark_refresh_preserves_only_unconfirmed_local_intent`,
  `deleting_a_template_retires_its_dirty_mark_before_replay`,
  `deletion_preserves_different_marks_and_refuses_writer_loss`, `session_confirmations_have_an_independent_replay_slot`,
  `quick_launch_naming_has_an_independent_replay_slot`, `terminal_link_click_uses_the_shared_queue_and_default`,
  `selection_mirror_and_recovery_overlay_keep_bounded_visits`,
  `returning_to_an_in_flight_value_still_sends_the_new_choice`.
- Possibly affected: the reauthentication test in `activity.rs` (around 445-481).

Some of these may already be gone or changed by `gui-saves-last-write-wins.md`.

### Validation

PR 1: `dprint check` on SPEC.md. PRs 2 and 3: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop` (the desktop renderer compiles nowhere else, and `PreferenceWrites`
serves both), and the narrowest `farhelm-ui` Rust selection through the recorder (`.agents/narrow-tests.md`) covering
the changed tests. PR 2 also runs the changed browser specs (`e2e/tests/auth.spec.ts`, `e2e/tests/welcome.spec.ts`) on
Chromium and WebKit through the recorder, per `.agents/validation.md`. `python -B scripts/check-test-sleeps.py` when
tests change.

## Fallbacks

Examples of departures that need the scope reassessment: a reload helper of its own instead of calling
`location().reload()`; reporting a failure that is silent today; touching the desktop authentication path; touching the
helm; production additions beyond the size rule.

If the reload cannot be made to work in the browser renderer at all, that is a block: the spec asks for a literal reload
or as close as possible, and what "as close as possible" means then is the maintainer's call.

Decisions to log: each item of the inventory removed, kept or changed and why, the shape of the reload, the
build-mismatch remount, whether PR 3 was split, and the measured production lines added and removed.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-sign-in-preservation-sweep-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: the review gate keeps the
model and effort this file demands, and the scope reassessment runs at your own model and effort. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
"Sub-agents this file requires", requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from these rules and run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Split the work into a linear stack of reviewable PRs, one reviewable commit, stable bookmark and draft PR each, using
  the `jjstack` skill. The outline above suggests the slicing; err on the side of bite-sized PRs.
- The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/sign-in-preservation-sweep/<nn>-<short-name>` (for example
  `plan/sign-in-preservation-sweep/01-best-effort-spec`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and changelog fragments per root `AGENTS.md`, "Changelog fragments".
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file, as is `releasing/changelog.d/`: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`) at high
effort, launched as a native sub-agent where your harness serves that model and otherwise through
`$scode-harness-shellout` (that skill owns the launch mechanics). The prompt carries the full charter, because the
reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, the out-of-scope list, and the outline.

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name, an inventory item that is already gone) is
corrected to the truth. If you find more preservation machinery than the inventory listed, removing it is within scope
when it only serves keeping or resending a failed save, or surviving a browser remount that the reload now replaces.
Anything whose removal would change behavior the out-of-scope list protects, or that the desktop app's own sign-in gate
needs, stays.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing everything that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
