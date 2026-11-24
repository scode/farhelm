# Hide non-actionable terminal file hovers

Written against main at f6d8d8bb2 on 2026-11-24. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Immediate"). Verbatim:

> **No hover for a detected file missing on remote.** When hovering over what Farhelm detects as a file, if it does not
> exist on remote, just silently ignore and do not show any hover at all.

The maintainer expanded the scope during planning:

> also for "undownloadable directories" and other things that are just not actionables, not sure what the full set it.
> point is don't spam the user wit hovers that aren't useful.

In product terms, sharpened by the maintainer's planning decisions: hovering over a detected printed file path in a
terminal checks the session's host silently, with no `Checking…` card, no underline, and no pointing hand while the
check is in flight. Only when the host confirms the path is a downloadable regular file (`DownloadFileStatus::Ready`) do
the underline, pointing hand, and hover card appear. Every non-actionable outcome — missing files (`NotFound`),
directories (`Folder`), non-regular files (`NotRegular`), files over the 100 MB limit (`TooLarge`), unreadable files
(`NotReadable`), and failed checks (`TransferError`, such as an offline host or timeout) — stays completely silent with
no hover card, no underline, and no pointing hand. Web links (`http`/`https`, printed or OSC 8) are unchanged.

### Acceptance criteria

1. **Silent checks and ready-only hover + underline for printed file paths:**
   - Hovering over a printed file path (`LinkKind::Path`) starts the host stat check silently: no popover opens, no
     `Checking…` spinner or text renders, no underline is drawn on the terminal text, and no pointing hand cursor is
     shown while the check is in flight.
   - If the host answers `DownloadFileStatus::Ready`, the underline on the terminal cells, the pointing hand cursor, and
     the file hover disclosure popover (`id:terminal-file-disclosure`) appear together, preserving the existing
     three-line layout (file icon + name, `host:directory`, decimal size + color tier + click/open hints).
   - If the host answers any non-`Ready` status (`NotFound`, `Folder`, `NotRegular`, `TooLarge`, `NotReadable`) or the
     stat fails (`TransferError`), nothing is shown: no popover, no underline, and no pointing hand.
   - Web links (`LinkKind::Text` and `LinkKind::Osc`) keep their existing immediate underline, pointing hand, and
     destination disclosure behavior unchanged.
2. **Existing cancellation, redraw, click-safety, and staged-hover invariants hold:**
   - Leaving a hovered printed path, scrolling it away, or rewriting its text/cells cancels any in-flight stat request
     and immediately clears any ready underline and popover. A redraw that preserves the same `TerminalLink` (cells and
     path text) keeps the in-flight, `Ready`, or settled non-actionable check without re-statting the host.
   - Click authorization stays fail-closed: only a `Ready` check over unchanged link text whose rendered disclosure
     popover is fully visible and uncovered (`disclosure::visibility` via `Drawn`) admits a click. Pressing before the
     `Ready` panel has painted, or on any non-ready path, does nothing (silent).
   - Focus-layer suppression (no popover while a dialog/menu holds focus), `refusal_waits_for_hover` (which also clears
     the check so moving to another cell checks afresh), `display_covers_pointer`, and Escape dismissal (`dismissed_at`)
     continue to work cleanly.
   - Staged runs (`agent_startup::staged_file_stat`, used by docs screenshots such as
     `scripts/docs-shots/work-in-a-session.py` and `HoverStagedTerminalFile`) resolve immediately and still show the
     underline, pointer, and hover card when staged as `Ready`.
3. **Clean simplification of unreachable refusal/checking hover UI (no backend/protocol churn):**
   - Remove unreachable checking/refusal/failure hover-card rendering and spinner state in
     `crates/gui/farhelm-gui/src/terminal/files.rs` (including `FileLinks::spinner`, `Hover::asked`, `SizeTier::Danger`,
     `CHECKING`, `SPINNER_SIZE`, `NO_HELM_CLIENT`, non-`Ready` `status_words` arms, `file_icon`'s status argument, and
     `hints`'s `Option` wrapper). Keep `SizeTier::Neutral` (still reachable for `Ready` with `size: None`) and
     `failure(&TransferError)` (used by `crates/gui/farhelm-gui/src/terminal/files/download.rs` for top-of-terminal
     download errors).
   - Leave `farhelm-proto`, `farhelm-helm`, and `farhelm-supervisor` stat routes and `DownloadFileStatus` unchanged (no
     protocol bump).
4. **Specs, tests, goldens, changelog fragment, and TODO cleanup:**
   - Update `SPEC.md` ("Terminal experience", file-path bullet), `SPEC_impl.md` (file-hover paragraphs), and
     `docs/gpui/parity/terminal.md` where they describe immediate underlining, `Checking…`, or refusal hover cards.
   - Update tests in `crates/gui/farhelm-gui/src/terminal/files/tests.rs` (including underline assertions on the
     existing `Harness` scenarios without mirroring them in `attached/tests.rs`) plus a focused standalone
     `TerminalView` unit test in `crates/gui/farhelm-gui/src/terminal/tests.rs` for `path_hover_ready` gating and reset.
   - Update `files::golden_displays` to render only the reachable `Ready` size tiers (small, medium, large), update the
     scene descriptions in `docs/gpui/screenshot-scenes.json` and `crates/gui/farhelm-gui/src/goldens/terminal.rs`,
     re-capture and publish the `terminal-file-disclosures` golden via `scripts/check-gui-goldens.py --update` and
     `scripts/gui-goldens.py publish`, and verify no unintended changes to other golden scenes.
   - Add a `kind: changed` changelog fragment under `releasing/changelog.d/` and remove the covered `TODO.md` entry
     (`No hover for a detected file missing on remote.`).

## Decisions already made

### Maintainer decisions (binding)

- **Outcomes to hide:** Hide all non-actionable outcomes (`NotFound`, `Folder`, `NotRegular`, `TooLarge`, `NotReadable`,
  and failed checks) — show hover only for downloadable (`Ready`) files.
- **Behavior while checking and for the underline:** Wait silently — show neither `Checking…` nor underline until
  confirmed showable (`Ready`).
- **Review gate:** Single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- **Release note:** Settled verbatim by the maintainer in `## Release note` below.

### Binding repository constraints

- `SPEC.md` ("Terminal experience", file-path bullet) and `SPEC_impl.md` (file-hover section): only a fully visible,
  uncovered hover disclosure for unchanged link text authorizes a click. When the popover opens upon reaching `Ready`,
  `Drawn` must start unset until the `Ready` panel actually paints so a click before paint fails closed.
- `crates/gui/farhelm-gui/src/terminal/files/download.rs` uses `files::failure(&TransferError)` for top-of-terminal
  download errors; keep it.
- `scripts/docs-shots/work-in-a-session.py` and `crates/gui/farhelm-gui/src/terminal.rs` (`hover_staged_file`) rely on a
  staged `Ready` file hover opening `id:terminal-file-disclosure`.
- Golden reference images live in the external store managed by `scripts/check-gui-goldens.py` and
  `scripts/gui-goldens.py`; manifest `crates/gui/farhelm-gui/tests/goldens.txt` is updated by `publish`, never by hand.

### Planner-chosen implementation shape

- **Mirror `AttachedTerminal::request_tunnels` for file checks (no duplicate tracking field):** Do not add a mirror
  hover-tracking field to `AttachedTerminal`. Expose `FileLinks::tracking(&self) -> Option<&TerminalLink>` and call a
  small `follow_files` helper at the top of `place_disclosure` beside `self.request_tunnels(cx)`: compute
  `wanted = view.hovered_link().filter(|link| link.file_path().is_some() && !self.refusal_waits_for_hover)`, and defer
  `files.follow(wanted.cloned(), self.platform, cx)` only when `self.files.read(cx).tracking() != wanted`. Keep
  `follow_files` separate from `request_tunnels` because their caching, retry, and cancellation contracts differ. Gate
  `disclosed_link` for `LinkKind::Path` on `self.files.read(cx).ready_for(link)`.
- **Snapshot `FileDisclosureView` (parallel to `DisclosureView`):** Because a file popover now opens only once a
  `FileStat` is already `Ready` and never mutates in place while open, have `AttachedTerminal::open_layer` read the
  ready `&FileStat` from `self.files.read(cx)` and instantiate a small `FileDisclosureView` holding
  `shown.drawn.clone()`. This removes `Hover::drawn`, `hover.drawn.set(None)`, `impl Render for FileLinks` (and its
  empty-`div()` fallback), and dummy `Hover` construction in `golden_displays`, makes fail-closed `Drawn` true by
  construction, and lets `files/tests.rs` reuse `AttachedTerminal::disclosure_drawn()`. Keep `FileDisclosureView`
  separate from `DisclosureView`.
- **Tight `FileLinks` state:** Collapse `Check` to `Checking | Ready(FileStat) | NotActionable`. Keep `self.hover`
  populated as `Check::NotActionable` on non-`Ready` answers so unchanged redraws do not re-stat missing files or
  directories; call `cx.notify()` in `answered()` only when the outcome is `Ready`.
- **Minimal `TerminalView::path_hover_ready` gating:** Add `path_hover_ready: bool` (default `false`) to `TerminalView`
  and gate `underlined_rows()` for `LinkKind::Path` on it while leaving web links (`LinkKind::Text`, `LinkKind::Osc`)
  unconditionally underlined. Push `view.set_path_hover_ready(ready, cx)` inside the existing `AttachedTerminal::render`
  `view.update(...)` block beside `set_cursor_phase`, reading from `self.files.read(cx).ready_for(link)`. In
  `TerminalView::update_shown`, reset `path_hover_ready = false` only when `hovered_link` actually changed, and keep
  notifying whenever `hovered_link` changes (even while `underlined_rows()` stays empty) so `AttachedTerminal` invokes
  `follow_files`.
- **Keep backend untouched:** `farhelm-supervisor`, `farhelm-helm`, and `farhelm-proto` need no changes.

## Outline

### Existing mechanisms reused

- `crates/gui/farhelm-gui/src/terminal/attached.rs` (`AttachedTerminal::place_disclosure`, `request_tunnels` pattern,
  `disclosed_link`, `open_layer`, `points_at_link`, `_files` subscription, `click_visibility`, `disclosure_drawn`,
  `refusal_waits_for_hover`, `display_covers_pointer`).
- `crates/gui/farhelm-gui/src/terminal/files.rs` (`FileLinks::follow`, `answered`, `ready_for`, `shows_pointer`,
  `answer_lines`, `golden_displays`).
- `crates/gui/farhelm-gui/src/terminal.rs` (`TerminalView::hovered_link`, `underlined_rows`, `set_hover`,
  `update_shown`, `render`).
- `crates/gui/farhelm-gui/src/terminal/disclosure.rs` (`Drawn`, `witnessed`, `visibility`, `surface`, `DisclosureView`
  pattern).

### PR stack

1. **`plan/hide-non-actionable-file-hovers/01-ready-only-file-hovers`** — Hide non-actionable terminal file hovers and
   wait silently while checking:
   - Wire `follow_files`, `disclosed_link` ready gating, and `FileDisclosureView` construction in
     `crates/gui/farhelm-gui/src/terminal/attached.rs`, and push `set_path_hover_ready` in `AttachedTerminal::render`.
   - Add `TerminalView::path_hover_ready` gating in `crates/gui/farhelm-gui/src/terminal.rs` (resetting only when
     `hovered_link` changes) so printed paths underline only once confirmed `Ready`, while web links underline
     immediately.
   - Simplify `crates/gui/farhelm-gui/src/terminal/files.rs` (`Check::{Checking, Ready, NotActionable}`,
     `FileDisclosureView`, drop unreachable checking/refusal/failure hover rendering and spinner state, keep
     `SizeTier::Neutral` and `failure` for `download.rs`, narrow `golden_displays` to the reachable `Ready` tiers).
   - Update `crates/gui/farhelm-gui/src/terminal/files/tests.rs` (including underline assertions on the existing
     `Harness` scenarios) and add a focused unit test in `crates/gui/farhelm-gui/src/terminal/tests.rs`.
   - Update `docs/gpui/screenshot-scenes.json` and `crates/gui/farhelm-gui/src/goldens/terminal.rs`, re-capture and
     publish `terminal-file-disclosures`, update `SPEC.md`, `SPEC_impl.md`, and `docs/gpui/parity/terminal.md`, add the
     `kind: changed` changelog fragment in `releasing/changelog.d/`, and remove the TODO entry from `TODO.md`.

## Fallbacks

- If `scripts/gui-goldens.py publish` cannot reach the external golden store (for example missing HTTPS credentials on
  the executing machine), block per `plans/.agents/executing.md`, "Executing one plan" (step 10), rather than leaving
  `crates/gui/farhelm-gui/tests/goldens.txt` out of sync with `golden_displays`.
- If a test outside `terminal/` depends on a removed refusal hover card in `golden_displays`, inspect it and update it
  to match the new `Ready`-only contract rather than keeping dead UI code.
- Scope-reassessment triggers (examples, not a blacklist): modifying `farhelm-supervisor`, `farhelm-helm`, or
  `farhelm-proto` stat behavior, adding a client-side stat cache, or changing web-link hover/underline behavior.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-hide-non-actionable-file-hovers-log.md` in the parent directory of the checkout you run in, derived as
that section says, and the watchdog's status file is `farhelm-plan-hide-non-actionable-file-hovers-watchdog.json` beside
it. Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent
scratch directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation":

- PR 1:
  - Targeted `farhelm-gui` tests covering `terminal::files::`, `terminal::tests::`, and `terminal::attached::tests::`
    through `python3 scripts/record-test-run.py --runner nextest ...`.
  - Golden visual review and publish per `.agents/validation.md`: first targeted
    `python3 scripts/check-gui-goldens.py --scene terminal-file-disclosures` (and inspect any generated changes page),
    then a full `python3 scripts/record-test-run.py ... -- python3 scripts/check-gui-goldens.py --update` to verify no
    unintended scene changes, then `python3 scripts/gui-goldens.py publish` and verify
    `python3 scripts/check-gui-goldens.py --scene terminal-file-disclosures` passes against the updated manifest.
  - `cargo fmt --all -- --check`, `cargo clippy -p farhelm-gui --all-targets -- -D warnings`,
    `python3 scripts/check-gui-lints.py`, `python3 releasing/check-changelog.py format`, `dprint check` on changed
    Markdown, and `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`.

Apply `.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live
Farhelm install on this machine (root `AGENTS.md`, "The live install is off-limits").

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
- The last PR of the stack removes the TODO.md entry this plan covers ("No hover for a detected file missing on
  remote."), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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
alternatives considered: in particular those named under Planner-chosen implementation shape and Fallbacks above, and
every review finding you decided not to follow. The maintainer will ask for these later.

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

Hovering over a detected file path in a terminal now shows a hover card only when the file can actually be downloaded,
instead of popping up errors for missing paths, folders, or other non-downloadable targets. This should avoid the
feeling of "hover spam" and hopefully reserve hover cards to cases where they are actually useful.
