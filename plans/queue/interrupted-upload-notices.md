# Report an interrupted upload after leaving its session

Written against main at b34ceee9f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Keep interrupted uploads visible after their session goes away.** The terminal leaves a line for each interrupted
> upload in `terminal/attachments.rs` (`Carried` and `prune_carried`), then the desktop can discard the lines when it
> learns the session is gone. SPEC.md's “Attachments” says upload failures must be visible. Preserve the file name and
> whether the upload was never sent or its result is unknown. Show the line on a surface that outlives the terminal,
> either when the user moves to another session or when removal is established. Reuse the notice center and account for
> a successful Delete or Replace behind a filter, when an incomplete list cannot prove absence. Cover leaving during a
> transfer and losing its source session without reopening it, including when there are no terminals left to draw it.
> Plan: `plans/queue/interrupted-upload-notices.md`.

### Where it stands (from planning research at main b34ceee9f)

In product terms: the user drops a file or pastes a screenshot into a session's terminal and, while it uploads, opens
another session. Switching sessions tears down the old session's terminals, which cancels the upload; that is by design
and stays. The terminal writes a line ("Attaching foo.png was interrupted before it was sent, so nothing was uploaded",
or "… was interrupted — whether it was attached cannot be told, and no path was inserted"), but it is only stored under
the old session for the next time one of its terminals shows. Until the user goes back, nothing says the upload stopped.
If the session is deleted or replaced, the line is thrown away once the list proves the session gone, never shown. Under
a status or host filter the list cannot prove it gone, so the line sits unseen; with no terminal open, nothing even
watches the stored lines. That breaks SPEC.md, "Attachments" (about line 3605): "Upload failures must be visible; an
attachment must never disappear silently."

- `crates/gui/farhelm-gui/src/terminal/attachments.rs`:
  - `Carried` (about lines 354-371) is an app-wide global from session key to already rendered lines (`HostSentence`),
    so the facts (file name, sent or not) are gone once a line is carried. `prune_carried` (about 372-398) drops a
    session's lines when the current store's listing proves it gone; it runs only from a terminal's attachments
    (`Attachments::new`, about 467-478), so with no terminal open nothing prunes.
  - `hand_over` (about 660-690) runs when a terminal goes away: it carries the strip's failure lines plus one line per
    running or waiting upload (`interrupted_line`, about 764). Only the running upload can have been sent; waiting ones
    never were. The interrupted texts are `INTERRUPTED_UNSENT_TEXT` and `INTERRUPTED_UNKNOWN_TEXT` (about 255-266).
    Upload names are already `HostText`.
  - `take_carried` (about 693-706) gives a session's lines to its next terminal; `terminal/set.rs` (about 618-629) picks
    which terminal of the set shows them; `terminal/attached.rs` (about line 1804) calls `hand_over`.
- `crates/gui/farhelm-gui/src/session_pane.rs` (module docs about 13-18, `sync` about line 403): one set of terminals,
  remade on every change of selection. The open session is `local_state::selected`
  (`crates/gui/farhelm-gui/src/local_state.rs`, about 40-58), an app-wide global that survives a reset when the new
  snapshot still has the session.
- `session::listing_proves_gone` (`crates/gui/farhelm-gui/src/session.rs`, about 270-278): only a complete, unfiltered,
  uncut, available listing proves a session gone.
- The notice center, `crates/gui/farhelm-gui/src/notices.rs`: `Notice` is `Copy` with fixed app text
  (`text(self) -> SharedString`); one variant already carries a flag (`LateAnswer { always }`). `post` ignores a notice
  already showing; `withdraw`; `posted`. Notices are app-wide and survive a reset (the connection-lost notice is posted
  because of one). Banners are drawn in `crates/gui/farhelm-gui/src/main_view.rs`: the posted list (about 182-196),
  `is_banner` (about 405-414), `banner` (about 431-478) with Dismiss, whose element id is formatted from the notice's
  `Debug`.
- `Banner::new` takes any element (`crates/gui/farhelm-gui/src/components/banner.rs`, about line 28), so a
  `HostSentence` (`crates/gui/farhelm-gui/src/host_text.rs`) can be its content: the one way the app's words and host
  text share a sentence.
- The Delete notice band's docs (`crates/gui/farhelm-gui/src/session_list/delete_notice.rs`, about 38-44) explain why it
  did not use the notice center: "The notice center carries `Copy` notices in the app's own words and keeps them across
  a reset … while this notice is host text that must go with the UI session, so carrying it there would change that
  foundation beyond its one extension point, a new variant." `docs/gpui/foundation-ownership.md`, "Notice center" (about
  443-448) names that one extension point.
- The banner golden: scene `notice-banner` (`crates/gui/farhelm-gui/src/goldens/mod.rs`, about line 585; drawn by
  `notice_banner` in `goldens/foundations.rs`, about line 914), its hashes in
  `crates/gui/farhelm-gui/tests/goldens.txt`, its title in `docs/gpui/screenshot-scenes.json` (about line 2589).
- Tests in `crates/gui/farhelm-gui/src/terminal/attachments/tests.rs`:
  `a_closed_tab_reports_its_cut_off_uploads_in_the_agent_terminal` (about 1095),
  `a_failure_and_a_cut_off_upload_outlive_their_terminal` (about 1120, which says an upload cut off by leaving the
  session must still be there when its terminal is back), `a_reset_reports_a_cut_off_upload_in_the_new_agent_terminal`
  (about 1163, the test SPEC_impl.md names), and `after_a_reset_the_lines_of_a_session_gone_from_the_listing_go` (about
  1199). That harness selects no session and can show terminals of two sessions at once, which the real app never does.
- Specs: SPEC_impl.md, "One failure path: the reset" (about 488-495: the carried lines, and "They go when the new
  terminal takes them, or once the current UI session's store proves their session gone"; about 512-514: the named reset
  test), and "Notices: the notice center" (about 1408-1419: "A new notice is added only where SPEC.md requires one, and
  each is drawn in an existing style"). SPEC.md, "Attachments", is the requirement.

### Acceptance criteria

1. Whenever a session with carried interrupted uploads is not the open session, those uploads leave `Carried` and are
   posted to the notice center at once: switching to another session, Replace (which opens the replacement), a Delete
   whose automatic selection moves on, the session vanishing, or no session being open. No proof of removal is needed,
   so a filter, a cut listing or the absence of any terminal changes nothing.
2. While the session is still the open one, nothing changes: a reset keeps the same session open, and its new terminal
   shows the interrupted line as today. `a_reset_reports_a_cut_off_upload_in_the_new_agent_terminal` keeps passing in
   substance, and no notice is posted for it.
3. The notice names the file, the session's title drawn as host text, and whether the upload was never sent or its
   outcome cannot be told, in the wording under Planner proposals. It is drawn as a banner in the notice center's
   existing banner style with Dismiss, stays until dismissed, and survives a reset.
4. An upload reported by a notice is not also shown when the user returns to the session.
5. Several interrupted files of one session and one outcome collapse into one banner that still names them: the names
   listed, and beyond a few, the first few and "and N more files".
6. Failure lines that a strip already showed are carried and pruned exactly as today; only interrupted uploads move.
   Switching sessions still cancels uploads.
7. `Notice` changes only as far as a notice carrying these values requires, and the docs that describe the notice center
   stay true: `notices.rs`'s module and enum docs, SPEC_impl.md's "Notices: the notice center" and the carried lines'
   paragraph of "One failure path: the reset", the Delete notice band's reasoning about the notice center, and
   `docs/gpui/foundation-ownership.md`'s "Notice center" extension point.
8. Tests cover: leaving during a transfer (the running upload's notice says its outcome cannot be told) and with an
   upload waiting (never sent); the source session deleted or replaced afterwards, under a filter, without reopening it,
   with the notice still showing; no session open when the terminal goes; returning to the session showing no line for a
   reported upload; the reset case still delivering to the new terminal; a notice surviving a reset; Dismiss withdrawing
   it.
9. A golden of the new banner in both appearances, captured through the goldens check, its pictures published with
   `python3 scripts/gui-goldens.py publish` and the manifest change committed with the PR, before any rebase, as
   `.agents/validation.md` describes; the visual review passes.
10. The PR carries a changelog fragment and removes the TODO entry.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **When:** post the notice as soon as the open session is not the interrupted upload's session; no removal proof. A
  reset keeps the same session open, so today's in-terminal line still shows after a reset.
- **Where: extend the notice center**, in the maintainer's words because "it's about your own action and should survive
  a reconnect". This means a notice that carries data, changing `Notice`'s `Copy` and fixed-text assumption in the
  smallest way, and updating SPEC_impl.md, "Notices: the notice center" (and the Delete notice band's doc comment where
  it becomes inaccurate).
- The maintainer settled where and when, nothing more. The scope limits, what the notice says (naming the session
  included), that it stays until dismissed, and how several files collapse are the planner's proposals below.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort, plus the mandatory Opus visual review
  (see "Visual review").
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- SPEC.md, "Attachments"; SPEC_impl.md, "One failure path: the reset" and "Notices: the notice center", which this plan
  changes where acceptance criterion 7 says; SPEC_impl.md, "Host-supplied text" (`host_text.rs`'s module docs) for the
  session title and file names.
- `docs/gpui/foundation-ownership.md`: the notice center is a refused foundation whose extension point is a new variant;
  the maintainer's decision authorizes going beyond it, and the extension point text is updated to match.
- `.agents/test-authoring.md` for every test added or changed; the GPUI goldens check (`.agents/validation.md`).
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", and its "Instructions by
  path" for every path you touch.

**Planner proposals:**

- Scope: interrupted uploads only, not earlier failure lines already shown on a strip (they were visible once). No
  second showing when the user returns to the session, so nothing is said twice. Switching sessions still cancels
  uploads; letting an upload finish after its terminal goes is a bigger behavior change SPEC.md does not ask for.
- What the notice says: the file name or names, the session's title, and whether the upload was never sent or its
  outcome cannot be told. The names and the title are drawn as host text inside the sentence; the rest is the app's
  words. It is drawn in the notice center's existing banner style (SPEC_impl.md, "Notices: the notice center", requires
  an existing style) and stays until dismissed, like the connection-lost notice.
- Naming the session is kept on purpose: once the user has left more than one session, "Attaching foo.png was
  interrupted" alone does not say where to retry. A simpler shape was considered and declined: the notice carrying the
  terminal's own rendered interrupted line, with `Carried` only flagging which lines are interrupted uploads. It would
  drop the structured entries and the title capture, but it names no session and gives one banner per file. If you
  revisit it, log it as a DECISION against this reasoning.
- Keep `Carried` entries structured for interrupted uploads (name, sent, session title) and render them where they are
  shown; the strip's failure lines stay rendered sentences.
- Capture the session's title when the upload is queued, from the current store's row for the session. By the time a
  Delete moves the selection, the reply's changes have already removed the row, so reading it when the terminal goes
  would find nothing. The title shown is then the one at the drop.
- One app-wide watcher, not one per terminal, so it works with no terminal open: it observes `Carried` and the open
  session (`local_state`), and moves every interrupted entry of a session other than the open one into notices. Put it
  next to `Carried` in the terminal module, installed once with the app's other globals or by the main view; your call.
- The exact sentences: "Attaching foo.png to <session title> was interrupted before it was sent, so nothing was
  uploaded." and "Attaching foo.png to <session title> was interrupted — whether it was attached cannot be told."
  (joined with a dash, as the terminal's own interrupted line is). Several files keep their names: "Attaching foo.png,
  bar.png and baz.png to <session title> was interrupted before they were sent, so nothing was uploaded." and "Attaching
  foo.png and bar.png to <session title> was interrupted — whether they were attached cannot be told." Beyond three
  names, list the first three and the rest as a count: "Attaching foo.png, bar.png, baz.png and 4 more files to
  <session title> …". The words deliberately do not say "when you left it": the app can also move the selection itself
  (a Delete of the open session moving on, a reset whose listing no longer has the open session), and the sentence must
  stay true then.
- One notice per session and outcome per move. Only the running upload can have been sent, so a session's move usually
  yields one banner, and two at most when one upload was running and others waited. A repeat identical to a notice
  already showing (same files, session and outcome) is absorbed by the existing `post` rule, which ignores a notice
  already showing; build nothing more (no merging into a showing notice, no unique id to force a second banner).
- The smallest change to `Notice`: drop `Copy` (keep `Clone`) and add a variant whose payload carries the title and file
  names as `HostText` and the outcome (an `Rc` keeps clones cheap). A side table keyed by a `Copy` id was considered and
  is worse here: `text` and `banner` take no `App`, so they could not look the data up. The fixed-text variants keep
  `text`; `banner` draws the new one as a `HostSentence`. Give its Dismiss an element id that does not grow with the
  file names if the `Debug`-based id gets unwieldy.
- SPEC.md, "Attachments": add one sentence saying that an upload cut off by leaving its session is reported in a notice
  naming the file and the session, which stays until dismissed. It is product behavior a reader of SPEC.md should find.
- Pitfalls found in planning:
  - The attachments test harness selects no session. An app-wide watcher would treat every carried entry there as not
    open, which would break the reset tests. Make the harness select its session as the app does, rather than weakening
    the watcher. `a_failure_and_a_cut_off_upload_outlive_their_terminal` states the old rule for leaving; restate it to
    the new one (the failure line still waits for the session; the interrupted upload goes to a notice).
    `after_a_reset_the_lines_of_a_session_gone_from_the_listing_go` shows two sessions' terminals at once; keep its
    point (a session's strip failure lines are pruned once it is proven gone) in a shape the real app can reach.
  - Check that a reset never shows the watcher a moment with no open session or a different one: the selection must
    survive the inert window, or a reset would post a notice and lose the in-terminal line.
  - Check that the key an upload's terminal is filed under is the key `local_state` selects.
  - Removing `Copy` touches every place a notice is passed by value more than once (about twenty files use `Notice`):
    `is_banner(*notice)`, the Dismiss closure, tests comparing posted notices.
- The golden: one new scene beside `notice-banner` drawing both outcome banners stacked, one with a single realistic
  file name and one listing several, with a realistic title, in both appearances; add its title to
  `docs/gpui/screenshot-scenes.json`. `notice-banner` itself should not change.
- Fragment, `kind: fixed`, along the lines of: "If you switch away from a session while a file is still uploading to it,
  a notice now tells you the upload was interrupted, even if that session is then deleted or replaced."

## Outline

1. `plan/interrupted-upload-notices/01-interrupted-upload-notice`: the structured carried entries, the watcher, the
   notice variant and its banner, the doc and spec updates, the tests, the golden, a changelog fragment, and the TODO
   entry's removal. `fix:` (for example `fix: report an interrupted upload after leaving its session`), with a fragment.

Whether to split this into two PRs is your judgment: first letting notices carry data (dropping `Copy`, with no behavior
change, as `refactor:` with no fragment), then the fix on top. That split builds on itself and is not churn; log the
choice either way. A split uses `plan/interrupted-upload-notices/01-notices-carry-data` and
`plan/interrupted-upload-notices/02-interrupted-upload-notice`, the last removing the TODO entry.

## Fallbacks

- If the session's row cannot be read when an upload is queued, draw the notice without the title ("Attaching foo.png
  was interrupted before it was sent, …"), log it, and say so in the report.
- If keeping `Copy` turns out simpler than this file expects after all, without a side table that needs `App` in the
  drawing path, take it and log why.
- A golden that changes for an existing notice or another scene is a red flag: investigate before accepting it.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-interrupted-upload-notices-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-interrupted-upload-notices-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. Never take a state-changing action against the live Farhelm install on this machine (root
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/interrupted-upload-notices/01-interrupted-upload-notice`, or the two named under Outline if you split.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Keep interrupted uploads visible after their
  session goes away"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
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

### Visual review

The maintainer also demands a visual review for every PR that changes what the desktop app draws: a fresh-context
`claude-opus-5-5` agent at `high` effort, launched natively where your harness can (otherwise through
`$scode-harness-shellout`), separate from the reviewer above. Give it headless renders of every changed scene in both
the light and the dark appearance (the GUI golden scenes, regenerated with
`python3 -B scripts/check-gui-goldens.py --update`, and the golden changes page that run writes), the design and wording
this file proposes (there is no mockup), and the PR's diff. It must be satisfied that the rendered result matches the
intent of that design; loop until it reports no mismatch. A mismatch that breaks a Decision or a spec (an existing
banner style, host text drawn as such) is binding and is fixed; any other observation it makes, a disagreement with the
proposed wording included, is an input, handled like the reviewer's findings above. Do not write a launch command here
or in the log.

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
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

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
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate and,
where it changes what the app draws, its visual review, and the last removing the TODO.md entry this plan covers. Open,
not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
