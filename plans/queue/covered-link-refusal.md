# Refuse link clicks when another surface covers their details

Written against main at d480779c on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **A link click should be refused when another Farhelm surface covers its panel.** SPEC asks this; nothing checks it in
> the GPUI app. It needs a hit test in the layer manager, which today cannot see popovers that pass the pointer through
> (`layers::Popover::passes_pointer`; see `terminal/disclosure.rs`, `fully_visible`). Parity rows in
> `docs/gpui/parity/terminal.md`: terminal-files' covered-hover cases, terminal-links' covered-display refusal,
> grid-attach 1099 and 1464.

In product terms: when the pointer rests on a link or a file name in a terminal, a panel shows where it leads, and a
click opens it only while that panel is fully visible. SPEC.md also says the panel counts as visible only while no other
Farhelm surface (an approval card, a menu, a dialog) covers it, and the desktop app does not check that half yet. This
plan builds it, with the refusal's explanation raised above the card that caused it.

### Acceptance criteria

1. A click on a web link, a Cmd/Ctrl-click tunnel on a loopback link, or a click on a file link is refused when any
   Farhelm surface that blocks the pointer overlaps the link's details panel, in the frame last drawn when the click
   arrives. A file link's second check at the release (it re-checks today) uses the same test. A surface that does not
   overlap the panel, even one right beside it, does not refuse anything.
2. The test is a hit test against what was drawn, never a hand-kept list of covering surfaces (SPEC_impl.md requires
   this): every surface that blocks the pointer counts, whatever opened it. A popover that lets the pointer through (the
   drag-copy notice, say) does not count, and neither do display-only surfaces (a tooltip shown by keyboard focus, the
   launch progress pill, and the quick-launch sequence pill, which is drawn centred over the terminal while armed):
   those take no clicks. SPEC_impl.md records that gap, naming the sequence pill outright, since SPEC_impl.md cites it
   as the very surface the old app's hand-kept list missed.
3. The refusals use the old app's two strings, word for word: "cannot open: full link details are covered by another
   Farhelm surface; move the pointer and hover again" for links and tunnels, and "cannot download: full file details are
   covered by another Farhelm surface; move the pointer and hover again" for files. Both come from one template in the
   old app's `crates/farhelm-ui/assets/terminal-files.js` (around lines 485 to 491 at `24bc0580`;
   `git show 24bc0580:crates/farhelm-ui/assets/terminal-files.js`). When a panel is both outside the window and covered,
   the existing "not visible" wording wins, as in the old app. So the visibility answer, which the press admission and
   the file click's release check take as a yes/no today, carries why a click was refused.
4. While the pointer stays on the refused link, the one-line explanation is drawn above the approval cards, so a card
   that covered the panel never hides the reason it was refused (SPEC.md "Terminal links": the explanation "may cover
   the top of an approval card"). It is display-only, takes no clicks, and drops back to its ordinary place when the
   pointer leaves the link, as today.
5. SPEC_impl.md's "not built" notes for the cover check and the raised explanation (around its lines 758 to 760, 810 to
   812 and 3296 to 3301 at planning time) describe what is built; the layers module docs and
   `docs/gpui/foundation-ownership.md` describe the new layer order rung, if one is added. `vendor/README.md`, "The
   patch layers", gets a row and a bullet for the new GPUI patch layer. The parity rows in
   `docs/gpui/parity/terminal.md` (terminal-files' covered-hover cases, terminal-links' covered-display refusal,
   grid-attach 1099 and 1464) are updated to match.
6. Tests cover criteria 1 to 4. The rectangle's edge cases (partial overlap, an adjacent hit area that does not overlap,
   a hit area that does not block the pointer, clipping by a content mask) live in the GPUI patch's own test; if that
   test is a new test target, `.agents/validation.md`'s vendored GPUI test selection and its inventory name it, or it
   never runs. The terminal tests then need one cover per click kind (web link, tunnel, file link) refused with the new
   wording, a real approval card over the panel refusing, the panel's own card not counting as covering it, a
   pass-through popover not refusing, and the raised line drawn over a card and dropping when the pointer leaves. A
   screenshot golden pins the raised line over a card.
7. A changelog fragment per root `AGENTS.md`, "Changelog fragments", and the last code PR removes this plan's TODO.md
   entry.

## Decisions already made

Settled with the maintainer on the 2026-11-18 planning page, which showed a mockup of the raised explanation over an
approval card:

- **The raised explanation is in this plan.** Without it the most common case (an approval card over the panel) shows
  its reason under the very card that caused it. Rejected: deferring it.
- **Only surfaces that block the pointer count.** Display-only surfaces (a keyboard-focus tooltip, the launch pill) do
  not, and SPEC_impl.md records the gap; a tooltip that follows the pointer cannot be up while the pointer is on a link.
  Rejected: counting them too.
- **The old app's wording, word for word.** Rejected: one merged string.
- **Release note:** none needed (see the section at the end).
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the Decisions above.

**Binding repository constraints:** SPEC.md "Terminal links" and the file-link paragraphs after it (the cover rule and
the raised explanation); SPEC_impl.md's layer-manager section (the cover check is a hit test, never a hand-kept list);
`vendor/README.md` (the vendored GPUI tree changes only together with its patch file); root `AGENTS.md`, "Code rules",
"Changelog fragments", "Conventional Commits", "Tests and validation"; `.agents/test-authoring.md`.

**Planner proposals** (from reading the code at planning time; follow the code where it differs, logged as a DECISION):

- `terminal/disclosure.rs`'s `fully_visible` checks only that the panel lies inside the window's content area, and its
  doc says the cover half is not built. The press admits a click through `click_admitted` in `terminal/attached.rs`; a
  file click checks `disclosure_fully_visible` again at the release. `layers.rs` has a NOTE naming the missing hit test.
- What blocks the pointer: every popover card except a pass-through one, dialog veils, and approval cards (which the
  layers code requires to block, and which sit over the top of the terminal). Pills and tooltips have no hit area. The
  tunnel-error notice is page content drawn under the top row, so it can never cover the panel.
- GPUI's public `Hitbox::is_hovered_at` answers for one point of the last rendered frame; the frame's hit-test list is
  crate-private. The smallest exact test is a small patch to the vendored GPUI that asks whether any hit area inserted
  after a given one, and blocking the pointer, overlaps a rectangle (for example `Hitbox::is_occluded_within(bounds)`),
  with its patch file per `vendor/README.md`. The panel's recorder (`disclosure.rs`, which the files view uses too)
  records a plain hit area over the panel as it draws, held in `Drawn`, and `disclosure::fully_visible`, which both
  click paths already call, asks that question about it. The panel's own card inserts its hit area before its children
  and the recorder is its last child, so the panel's own card is not counted; confirm that in a test. Keep the layer
  manager's part thin (a function that replaces its NOTE and the ladder docs, as the TODO entry's "a hit test in the
  layer manager" asks): no record of popover bounds and no per-layer "covers" flag, which would drift toward the
  hand-kept list SPEC_impl.md forbids. The automation bridge's per-frame record is off unless the bridge runs, so the
  product cannot use it. Rejected: sampling a grid of points with `is_hovered_at` (inexact, misses small covers), and
  geometry kept by the layer manager.
- The refusal line today is page content at the top of the terminal (`attached.rs`, the `refusal` and `refusal_raised`
  state), which is exactly where an approval card sits. Criterion 4 needs a display-only copy above the approval region
  while `refusal_raised` holds. A close call: build it as a third slot on the layer manager's existing pill machinery
  (`Pill`, `ShownPill`, `PillId`, `hide_pill`: display-only, not a layer, anchored to an element and gone when that
  element is no longer drawn, invisible to `any_open`, Escape and Tab), anchored to the page's own refusal line so it
  lands exactly on it. Do not share the quick-launch sequence pill's single slot. Do not use a pass-through floating
  popover raised above the approval region: it would hold back an approval card's "Tunnel and open" press, a dialog
  opened over it would put its veil over the cards, and it would count as an open layer. Log the choice.

## Outline

1. `plan/covered-link-refusal/01-gpui-occlusion-test`, `fix:` (or `refactor:` if it carries no user-visible effect on
   its own; your call, logged): the vendored GPUI patch adding the rectangle test, its patch file, and its own unit
   test.
2. `plan/covered-link-refusal/02-refuse-covered-clicks`, `fix:`: the layer manager's covered-panel check, used by web
   links, tunnels and file links; the two refusal strings; tests; SPEC_impl.md edits for the cover check; the parity
   rows; the changelog fragment.
3. `plan/covered-link-refusal/03-raise-refusal-above-cards`, `fix:`: the raised explanation slot; its tests and golden;
   SPEC_impl.md, the layers module docs and the GUI ownership doc for the new rung; the TODO.md entry removal.

Fold PR 1 into PR 2 if the patch is a few lines; keep PR 3 separate.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
`farhelm-gui`, the GUI crate's terminal, disclosure, files and layers unit tests through
`python3 scripts/record-test-run.py`, the vendored gpui-pre test selection `.agents/validation.md` names for a GPUI
patch, the GPUI goldens, and `python3 releasing/check-changelog.py format`.

## Fallbacks

- If the vendored GPUI already offers a rectangle-against-later-hit-areas query that the planner missed, use it and skip
  the patch.
- If raising the explanation needs more than one new rung in the layer order (a second overlay system, say), that is a
  departure: run the scope reassessment.
- If some surface that should count as a cover (one a user would call covering) turns out not to block the pointer, do
  not add it to a list: record it in the report's things you should know.

Decisions to log: the exact query the patch adds and why, where the panel's hit area is recorded, the rung's place in
the layer order, the wording as copied from the old app, every test changed and why, and every review finding you
decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-covered-link-refusal-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-covered-link-refusal-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation".

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/covered-link-refusal/<nn>-<short-name>`,
  as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's entry for the release, not an instruction to
  you: each PR still leaves the fragment its type calls for, describing what that PR built, and where what got built
  departs from the release note (a renamed control, a part left out, a limitation it does not mention), the fragment's
  context paragraphs say how (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last code PR of the stack removes this plan's TODO.md entry.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above, each having passed its review gate, and the last code PR removing this plan's
TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close
the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
