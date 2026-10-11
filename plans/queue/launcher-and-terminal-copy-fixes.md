# Execute three triage outcomes: launcher retry key, template list errors, and copy in restored terminal snapshots

Written against main at 3f430cc4 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out three triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `fix code`, in one draft PR. Their
headings, which are also the feedback file names under `review_feedback_queue/`:

- `retrying-through-unchanged-permission-choice-launch-duplicate.md`
- `template-catalog-error.md`
- `restored-terminal-snapshots-lose-select-to-copy.md`

All three are small GUI behaviors in the New dialog (the launcher) and the terminal:

- The launcher gives each intended launch an idempotency key, so retrying after a lost reply cannot start a second
  session. Pressing Enter on an effort, permission or workspace-trust choice that is already selected re-applies it and
  launches, and re-applying it throws the key away even though nothing changed. If the first launch reached the helm but
  its reply was lost, that retry starts a second agent session for the same request. Choosing the already-selected
  harness or launch tab with Enter does the same.
- The launcher's `tl:` search lists saved templates. When reading the template list fails, the launcher treats it as an
  empty list: `tl:` shows nothing and offers no retry, unlike the model catalog, which says it is unavailable and offers
  Retry. A launch that opens the launcher on a named template (for example from the quick switcher) then reports that no
  template has that name and that it may have been deleted, which is wrong.
- When a terminal's recovery ends with the session taken over by another client, stalled, or refused, the terminal stays
  on screen as a frozen snapshot of what it last showed. Selecting text in that snapshot no longer copies it, because
  the snapshot keeps none of the live terminal's select-to-copy handling. Users who rely on select-to-copy lose it
  exactly when they may want to save the last output.

Each ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for
that outcome. Read the three entries and feedback files before starting. Where this file and a ledger entry seem to
disagree, the ledger entry wins and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria.
- Browser tests cover a lost-reply retry through Enter on an already-selected choice producing one session under one
  key, a failed template read shown in `tl:` search with a working retry, and select-to-copy in a restored terminal
  snapshot on Chromium and WebKit.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, adds a changelog fragment, and passed the review gate.
- The PR does not touch the vendored terminal library under `crates/farhelm-ui/assets/vendor`.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decision (2026-10-11):** the user asked for the whole review queue to be screened for findings
meeting all of these criteria: "it is CLEARLY a bug to be fixed. it's not ambiguous and doesn't need judgement that it
should be fixed given the nature of the product and its spec", "the fix seems reasonbly straight froward in the sense
that it won't add significant new complexity, require a re factor, etc.", and "its still relevant given current code vs
being stale due to code having changed"; and to triage that set as `fix code` "with an explicit complexity gate - it
must bounce back to me if during fixing it turns out to be more complicated and not fit the criteria". The decisions are
recorded in root `TRIAGE_OUTCOMES.md` (landed in #1896), one entry per feedback file, each with its own complexity gate
in its Completion criteria.

**The user's plan-time decisions (2026-10-11):** "let's use the planning system to schedule execution of the fixes".
Asked how to group the 31 outcomes, the user chose seven plans with one PR each; this plan is the launcher form and
terminal copy group, so its outcomes share one PR, overriding root `AGENTS.md`'s one-PR-per-outcome rule for these
outcomes only. Review gate: gpt-6.1-sol at high effort. No-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main by a fresh-context
planning review. Handling a complexity-gate trip by dropping that one outcome from the PR and shipping the rest is also
a planner proposal: it follows the precedent of earlier grouped triage plans and is how this plan honors the user's
"bounce back to me". Two further planner proposals: the maintainer is upgrading the vendored xterm.js separately, so
this plan leaves `crates/farhelm-ui/assets/vendor` alone and expects `crates/farhelm-ui/assets/terminal.js` to have
moved by the time it runs; and the snapshot copy fix reuses the copy policy already on main rather than writing a second
one.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Careful rebase; Execute
triage outcomes, as overridden above; Releases and the changelog; Agent scratch space; the rule against tests that
modify the test process's own environment variables), `plans/AGENTS.md` (Executing one plan),
`review_feedback_queue/AGENTS.md` (Lifecycle), `.agents/test-authoring.md` for test changes, `docs/test-sleep-check.md`
for the browser test changes, `docs/test-run-evidence.md` (recorded browser runs) and `.agents/narrow-tests.md` for
running and reproducing the Playwright specs.

## Outline

Line numbers drift; find the code by name. Verified on main at 3f430cc4.

Before writing code, rebase onto the latest main per root `AGENTS.md` (Careful rebase), reading in particular every
change to `crates/farhelm-ui/assets/terminal.js`, `crates/farhelm-ui/assets/copy-on-select.js` and
`crates/farhelm-ui/assets/vendor` since 3f430cc4. The xterm.js upgrade may rename what this outline names in
`terminal.js` (the tombstone map, `cancelReconnect`, the copy-on-select handlers) or change how xterm exposes the
selection and mouse modes; find the current equivalents by role. Changing the vendored library itself is a gate trip for
the snapshot outcome.

- Launcher retry key: `crates/farhelm-ui/src/list/create_form.rs`. The form holds `intent_key` as the key plus the
  `IntentBinding` it was minted for; the submit path reuses the key only when the binding still matches, and each
  field's change handler also clears it (the docstring "The intent key (PLAN_M3.md item 6), and what it is bound to"
  explains both edges). In the `LaunchControls` props, `on_effort`, `on_permissions` and `on_workspace_trust` set their
  value and then call `intent_key.set(None)` unconditionally. `enter_choice` in
  `crates/farhelm-ui/src/launch_controls.rs` applies the choice and then launches, also on a choice that is already
  selected, so Enter on the selected choice discards the key and the retry mints a new one. Clear the key only when the
  value actually changes: compare the incoming value with the signal's current value (read with `peek`) before setting.
  For permissions and trust, the comparison is on the value, not on the `*_is_explicit` flag: the binding carries the
  normalized value, and marking an unchanged value explicit does not change the request. Keep clearing on a real change
  rather than removing the clears and relying on the submit-time binding check: the YOLO confirmation under Launch is
  shown only while `intent_key` still holds the refused request's key, so a real change must keep retiring it.
  `choose_harness` and `choose_launch_tab` in `crates/farhelm-ui/src/list/create_form.rs` have the same shape and are
  wired to Enter through `enter_choice` with `launch_from_choice`; guard them the same way (the tab only when the tab
  changes; the harness only when the reconciled selection or the tab differs from what the form held). The ledger's
  Assessment left open whether those two can fire without changing the request; they can, so including them is a
  DECISION within the outcome's scope. If guarding the harness turns out not to be straightforward, guard only the tab
  (the most natural trigger, since Enter on the focused tab is almost always Enter on the selected one) and name the
  harness case in the report. Test: in `e2e/tests/terminal-create-idempotency.spec.ts`, next to the existing lost-reply
  test (which routes `**/api/sessions`, lets the first POST reach the helm with `route.fetch()` and then aborts it), add
  a structured-launch case: Codex harness, folder `/tmp`, a title, permissions chosen explicitly (as
  `e2e/tests/launcher-enter.spec.ts` does; `e2e/start-stack.sh` puts a fake `codex` on PATH, so the launch really
  starts), first POST lost, then focus the selected permission choice and press Enter. Assert both POSTs carried the
  same key and exactly one session with that title exists. A second case where Enter on a different value sends a new
  key is worth adding if cheap. The gate: new plumbing, a change to `IntentBinding` or the submit path, or the harness
  or tab paths needing more than the same guard.
- Template list errors: `crates/farhelm-ui/src/list/create_form.rs` reads templates in a `use_resource` that maps
  `api::fetch_templates` with `unwrap_or_default()`, under a comment saying a failed read offers none and nothing else
  depends on them; `crates/farhelm-ui/src/api.rs`, `fetch_templates`, already returns `Result` and its docstring says a
  failure leaves the caller with no templates rather than an error in the launcher. Keep the `Result` in the resource;
  derive the template list from the `Ok` value where the form now uses `unwrap_or_default` (the `tl:` names for search,
  `template_names_for_search_input`, and the initial-action path). While the search scope is `SearchScope::Template` and
  the read failed, show the failure with a Retry button that calls `clear()` and `restart()` on the resource, mirroring
  the model catalog's `create-catalog-error` block (a `role: "status"` message plus a retry button) and placed where the
  GitHub scope note `launch-composer-repository-note` appears for repository search. Other choices stay usable. The
  initial-action effect waits for the templates to load and then calls `template_edits`, which on an empty list reports
  "no template is named {name:?}; it may have been deleted"; with a failed read, set the form's template error to say
  the template list could not be read, with the reason and a hint to retry, instead.
  `crates/farhelm-ui/src/list/quick_switcher.rs` already shows "Couldn't load templates:" in its template mode and is
  the wording precedent. Update the `fetch_templates` docstring and the resource comment. Test in
  `e2e/tests/templates.spec.ts`, which already has a failing-templates route pattern (a 503 `route.fulfill` behind a
  toggle) and a `tl:` helper: fail the template GET, open the launcher, type `tl:`, assert the error and the retry
  button, stop failing, click retry, assert a saved template is listed. For the initial action, follow
  `e2e/tests/quick-switcher.spec.ts`, which counts template reads (the second is the launcher's): fail only the
  launcher's read and assert `.launch-composer-template-error` names the read failure rather than a missing template.
  The gate: retry needing anything beyond restarting the resource, or the error changing launcher behavior outside `tl:`
  search and the initial action.
- Copy in restored snapshots: `crates/farhelm-ui/assets/terminal.js`. When recovery ends in takeover, stall or refusal,
  `cancelReconnect` with outcome `"restore"` unmounts the island, shows the held terminal and records it in the
  `tombstones` map (the single place an entry is added); `buryTombstone` is the single place one is removed and
  disposed, and `unmountAll` buries them all. The live terminal's select-to-copy is `handleTerminalMouseDown` (records
  per press `trackingAtPress` from `term.modes.mouseTrackingMode` and `forced` from
  `window.farhelmCopyOnSelect.pressForcesSelection(ev, navigator.platform)`) and `handleCopyOnSelectMouseUp` (on
  document mouse-up, after a zero timeout, reads the selection and calls
  `copySelectionOnMouseUp({hasSelection, selectionText, trackingAtPress, forced})`, then `enqueueClipboardWrite`),
  removed by `stopCopyOnSelect`. The copy-on-click fix, which made `copySelectionOnMouseUp` take `trackingAtPress` and
  `forced`, is already on main in `crates/farhelm-ui/assets/copy-on-select.js`; reuse that function,
  `pressForcesSelection` and `enqueueClipboardWrite` as they are. In the restore branch, install a small snapshot-only
  handler on the held terminal: a mouse-down listener on its element recording `trackingAtPress` and `forced`, and a
  document mouse-up listener that acts only after a press on that element, re-checks after the zero timeout that the
  tombstone still holds this terminal, and copies through `copySelectionOnMouseUp` and `enqueueClipboardWrite`. Store
  its stop function in the tombstone entry and call it in `buryTombstone` before disposing the terminal. Omit the live
  terminal's drag-may-have-copied-nothing notice and its OSC 52 grace: no program is attached to a snapshot, so neither
  applies (a DECISION to log). The handler must not restore input, resize or connection callbacks. Do not restructure
  the live handlers to share code beyond the policy functions already in `copy-on-select.js`. Test: in
  `e2e/tests/terminal-reconnect.spec.ts`, the takeover-during-backoff scenario ends in a restored snapshot showing an
  echoed marker with no island; extend it or add a sibling that drags across the marker row (located through the DOM,
  since `e2e/tests/terminal-clipboard.spec.ts`'s helpers read the island buffer, which a snapshot no longer has) and
  asserts the copied text. On Chromium, grant clipboard permissions and read the clipboard back, as
  `terminal-clipboard.spec.ts` does. That spec skips WebKit because WebKit cannot grant clipboard read, so on WebKit
  only, replace `navigator.clipboard.writeText` with a recorder after the snapshot appears (the clipboard queue looks
  the method up at call time) and assert the recorded text. Log as a DECISION that the WebKit assertion is on the text
  written, not a clipboard read-back. If cheap, also assert through `window.__farhelmTombstones` (used by the
  "a-departed-tombstone-is-buried" test) that burying the snapshot removes the handler. The gate: having to restructure
  the live handler (its notice or OSC 52 grace), duplicating more than a small amount of policy, or changing the
  vendored library.
- Changelog: `fix:` with `kind: fixed`, for someone running Farhelm: retrying a launch by pressing Enter on a choice
  that is already selected no longer starts a second session; the New dialog's `tl:` search says when templates could
  not be loaded and offers Retry, and opening the launcher on a template no longer claims the template was deleted when
  the list could not be read; selecting text in a terminal left as a snapshot after another client took it over copies
  it again.

The three outcomes touch different code and do not interact, apart from sharing the Playwright stack and the
`crates/farhelm-ui` build.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`
(the touched Rust is in `farhelm-ui`). Run a focused `cargo nextest run -p farhelm-ui` selection through
`scripts/record-test-run.py` with `--tmux none` only if Rust unit tests change. Run
`cd crates/farhelm-ui/js-tests && node --test` only if `copy-on-select.js` changes, which this outline does not plan.
Run `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares, apply
`.agents/test-authoring.md`, run `dprint check` on the changed Markdown and
`python3 releasing/check-changelog.py format`.

All three fixes are browser behavior, so Playwright is warranted. Build with `cargo build` and
`cd crates/farhelm-ui && dx build --package farhelm-ui --platform web --release`, then run through the recorder from
`e2e/` on Chromium and WebKit with `--tmux required` and the pinned tmux, in the "Recorded browser runs" shape in
`docs/test-run-evidence.md`: `terminal-create-idempotency.spec.ts`, `launcher-enter.spec.ts` (as a regression check on
Enter in the launcher choices), `templates.spec.ts`, `quick-switcher.spec.ts` if it changes, and
`terminal-reconnect.spec.ts` narrowed with a grep to the touched tests plus the tombstone tests. Use
`.agents/narrow-tests.md` to reproduce any failure narrowly. No desktop check is needed unless the asset set changes, in
which case run `scripts/check-desktop-assets.sh`. Nothing here is macOS-only.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-launcher-and-terminal-copy-fixes-log.md` in the parent directory of the checkout you run in, derived as
that section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution fields on the PR) rather than starting over. If it does not exist, this is a
fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/launcher-and-terminal-copy-fixes/01-launcher-terminal-copy`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); `kind: none` with a one-line reason is right when nothing changes for
  someone running Farhelm. Validate with `python3 releasing/check-changelog.py format`.
- For every outcome the PR carries out, it removes the feedback file under `review_feedback_queue/` and its line in
  `review_feedback_queue/INDEX.md`, and updates that outcome's `TRIAGE_OUTCOMES.md` Execution field to `complete` with
  the jj change ID, bookmark and PR URL. Record the change ID and bookmark before creating the PR, then add the URL to
  the same change and push again; no separate bookkeeping PR. An outcome dropped by its complexity gate is handled as
  Complexity gate says instead.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Complexity gate

Every outcome in this plan carries the user's complexity gate, stated in its ledger entry and in the outline above: the
fix should stay about as small as assessed at triage. If one turns out to need significantly more (a new mechanism,
cross-component plumbing, a change well outside the files the outline names, a change to the vendored terminal library,
or a product or design decision), do not grow it and do not block the whole plan for it. Leave that outcome out of the
PR, keep its feedback file and index line, and in the same PR set its `TRIAGE_OUTCOMES.md` Execution field to `pending`,
waiting on the user, followed by what you found and the realistic options with a recommendation (see
`installer-startup-prune.md` in the ledger for the shape). Name it among the open questions in the plan's report. The
rest of the PR ships. Block per Unattended fallback only when every outcome in this plan was dropped, since then there
is no PR to deliver. Code that turned out to be already fixed on main is not a gate trip: record the outcome as
`discard` with "already fixed" and the fixing commit, per root `AGENTS.md`, and remove its queue item.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria: each outcome's `TRIAGE_OUTCOMES.md` entry, the outline above, and
the user's decisions. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim, as root `AGENTS.md` requires. Also ask the reviewer to check that a real change of effort, permission, trust,
harness or tab still retires the key and with it the YOLO question; that the snapshot handler is removed when the
snapshot is buried and restores no input or connection callbacks; that its forcing and mouse-tracking rules match the
live terminal's; and that a failed template read leaves the rest of the launcher usable. Where you disagree with a
finding, decide on the merits and log the DECISION. Do not write a launch command here or in the log; the
harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new helper module, a
change to a shared formatter or protocol, edits outside the files the outline names beyond tests and bookkeeping), and
whenever the same component has needed repeated corrective review rounds, run a fresh-context review through
galaxy-brain with this charter, supplying the ledger entries, the user's decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A departure that
the review does not find necessary and small is a complexity-gate trip, handled as above.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular guarding the key clears versus removing them, including the harness and tab
choices, the template error's placement and wording, the initial action's message on a failed read, leaving the drag
notice and OSC 52 grace out of the snapshot handler, how the clipboard is asserted on WebKit, what the careful rebase
found in `terminal.js` and how the outline's names were mapped, which browser specs ran, the Conventional Commit type
and changelog kind, every outcome dropped by its complexity gate and why, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

You never ask the maintainer anything in conversation while executing this plan: not a blocking question, not an
optional one, and not an offer of more work. Nobody is watching, and a question in conversation stalls the plan until
someone happens to look. Every fork is either decided within the agreed scope and logged as a DECISION, or it is a block
per `plans/AGENTS.md` (Executing one plan, step 10). Anything the maintainer should know, and any possible follow-up
(for example a similar key-clearing pattern elsewhere in the launcher), goes into the plan's report.

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is the complexity gate above. If current code or specs have
moved so that a recorded decision no longer applies, treat it like a gate trip for that outcome, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself. Block per `plans/AGENTS.md` (Executing one plan, step 10)
only when no outcome is left to ship or the shared PR itself cannot proceed without the user.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate, meets the acceptance criteria above, and passed the review gate, with every outcome's
`TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out outcomes removed. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
