# Demo video capture on the desktop app

Written against main at d480779c on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Rebuild the demo video's capture flow on the GPUI app.** Deferred past the GPUI migration on 2026-11-16. The README
> hero and docs-screenshot capture run on the GPUI screenshot tooling; the demo video (`scripts/readme-video.sh`,
> `docs/readme-video/`) still drives the deleted browser UI. Its `intent.md` is still the throwaway sample and needs a
> real design first. The rebuild then needs an in-app overlay for the pointer, captions and cards, and a frame recorder.

The maintainer's instruction when choosing it, verbatim:

> do this but only update the _mechanism_ and update the existing video in whateer way makes sense, its just a demo to
> demonstrate the mechanism for now so no need to ask me about content just get the mechanism working

So the goal is the capture mechanism: `scripts/readme-video.sh` records the demo video again, from the desktop app, with
the pointer, callouts, highlights, captions and title cards the old browser capture had. The video's design
(`docs/readme-video/intent.md`, the scenario, the transcripts) stays the throwaway sample it is, changed only where it
must be to stay true and work on the desktop app. A real design is out of scope.

### Acceptance criteria

1. `scripts/readme-video.sh` records the demo video from the GPUI desktop app on Linux, using the GPUI screenshot and
   agent tooling the README hero and docs screenshots use (`scripts/gui-agent.py`, the docs-shot helpers), against the
   existing staged scenario, and writes an H.264 MP4 plus one review still per mark, as the old flow did.
2. An overlay in the app draws the video's pointer (gliding to where the next click lands), callouts with a preferred
   side, highlights with an optional spotlight, captions, and title cards with fades. It exists only in builds made for
   the screenshot tooling (the `agent` feature or equivalent), never in a release build; it takes no pointer input, so
   clicks and the tooling's checks behave as without it; the capture script drives it through named actions. Callout and
   highlight placement is not reimplemented in the app: the capture script lays them out with the existing callout
   engine the docs screenshots use (`scripts/docs_callouts.py`, itself a port of the old browser overlay's look and
   placement rules) and sends the finished geometry; the overlay only paints and animates.
3. Cuts (a wait trimmed out of the video) work: the recorder records segments and joins them, so the finished video does
   not show the waits the script cuts.
4. The capture keeps the screenshot tooling's privacy checks (no private paths or names in any frame or still) and its
   crop of the Linux window frame (the 21 px border on every side and the 34 px title bar), reusing those helpers and
   constants rather than copying them.
5. The video's content changes only where the desktop app requires it: "right in the browser" in beat 4 becomes the app,
   file paths and comments that name the deleted browser flow are corrected, and beats that no longer work on the app
   are rewritten to realise the same intent. `intent.md` keeps its THROWAWAY SAMPLE note.
6. `docs/readme-video/SPEC.md` describes the new mechanism; `.agents/docs-media.md`'s demo-video section and its note
   that the refresh is unavailable, root `AGENTS.md`'s "Screenshots, demo video, brand marks" flow line ("The demo video
   refresh is unavailable until rebuilt for the GPUI app"), `docs/gui-agent.md` where it names the video, and
   `docs/gpui/foundation-ownership.md` (a row for the new files) are updated. `python3 scripts/check-agents-md.py` runs
   for the instruction-file changes.
7. Tests: Python unit tests for the recorder's segments, timing, encode command and marks; Rust tests that the overlay
   draws and takes no hit testing (the bridge refuses a click whose target another element covers, so this protects a
   real contract), and the default-feature build still compiles without it; the existing tooling tests
   (`scripts/test-gui-agent.py`, `scripts/test-docs-shots.py`) still pass. One live Linux run of
   `scripts/readme-video.sh`, with every still looked at and annotation placement fixed until each points at the right
   thing.
8. The executor hands the recorded MP4 and stills over in its report, saying the video only demonstrates the machinery
   (the THROWAWAY note). It commits no video and uploads nothing: the upload is manual (`.agents/docs-media.md`).
9. The last code PR removes this plan's TODO.md entry. Fragments per root `AGENTS.md`, "Changelog fragments": this work
   has no user-facing effect, so a fragment is needed only where a PR's type requires one, and then it is `kind: none`.

## Decisions already made

- **Mechanism only, no questions about content** (the maintainer's instruction above). Update the existing video in
  whatever way makes sense for it to work and stay true; do not design a new one.
- **Planner choices stated on the 2026-11-18 planning page, not objected to:** an in-app overlay in tooling builds only,
  driven by named actions; the screenshot bridge's existing recorder (`record start` / `record stop`), one recording per
  segment, joined in one ffmpeg pass; Linux at 1920×1080, since a Mac screen cannot fit a window that size; the executor
  hands the video over rather than publishing it.
- **Release note:** none needed (see the section at the end).
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the instruction above, and the Decisions.

**Binding repository constraints:** `.agents/docs-media.md` (read it in full before touching `docs/readme-video/` or
`scripts/readme-video.sh`; the design files change only when the maintainer asks, and the maintainer has asked for the
minimal changes above); `docs/readme-video/SPEC.md` (read it before touching anything under `docs/readme-video/`); root
`AGENTS.md`, "Frozen material" (the demo video's design), "Sharing the machine with other agents" (no fixed ports, paths
or names in the harness), "Code rules", "Conventional Commits", "Tests and validation"; `.agents/test-authoring.md`;
`vendor/README.md` if the vendored GPUI agent tooling must change.

**Planner proposals** (from reading the code at planning time; follow the code where it differs, logged as a DECISION):

- `scripts/readme-video.sh` still builds the deleted web UI and runs Playwright; the browser capture files
  (`capture.spec.ts`, `overlay.ts`, `recorder.ts`, `paths.ts`, `readme-video.config.ts`) were deleted in #3401.
  `git show 6ef88c9a4^:e2e/readme-video/<file>` shows them; the old `overlay.ts` (its `Director`) and `recorder.ts` say
  what the overlay and recorder did.
- The GPUI tooling already loads the video's scenario (`scripts/gui-agent.py`, `plan_scenario`;
  `scripts/test-gui-agent.py`). Every screenshot comes from the app's own renderer. The scenario-capture helpers the
  README hero uses (pointer parking, leak checks, the Linux frame crop, the size check) in `scripts/gui-agent.py` carry
  a comment saying the demo video should reuse them; `scripts/docs_shots.py` has a `Bridge` facade, and
  `docs_shots.annotate` already turns bridge boxes into content coordinates past the Linux frame.
- `scripts/docs_callouts.py` (#4126) keeps the old overlay's callout and highlight rules (`Callout`, `Highlight`,
  `wrap`, `place`, `layout`, `arrow`). Reuse its layout for the video; the one cost is turning its module constants into
  a docs look and a video look, as the old overlay had two themes. `scripts/test-docs-shots.py` must still pass.
- A Linux window at 1920×1080 content is 1962×1135 with its frame, and the headless display must fit it
  (`docs/gui-agent.md`).
- Review stills can be the newest recorded frame at each mark, cropped with the existing single-frame crop, so a still
  always shows what the video shows; the MP4 itself gets an ffmpeg crop built from the same frame constants. Your call;
  log it.
- The vendored bridge already records: `record start --fps N` writes PNG frames from the app's renderer and
  `record stop OUT.mp4` encodes them with ffmpeg. It has no timestamps and no pause, each start clears the previous
  recording, and its JSON gives the frame count but not the duration, so the harness times each segment itself, moves
  each segment's frames away after its stop, and checks the frame count. Nothing uses `record` yet.
- Synthetic input moves the app's own pointer position, so the overlay draws its pointer at the app's pointer position
  and eases toward it; the script hovers, waits for the glide, then clicks. The harness measures each target's box per
  beat and sends coordinates; review stills catch drift.
- The overlay is drawn above every layer from the root wrapper (`crates/gui/farhelm-gui/src/root.rs`), with no hit area.
- The lavapipe render rate at 1920×1080 is unmeasured: choose the frame rate after measuring it.

## Outline

1. `plan/demo-video-capture/01-video-overlay`, `test:` or `chore:` (tooling only; your call, logged): the tooling-only
   overlay and its actions, with Rust tests.
2. `plan/demo-video-capture/02-video-recorder`, same type rule: the segment recorder, the beats module replacing the
   deleted `capture.spec.ts`, the rewritten `scripts/readme-video.sh`, and Python tests.
3. `plan/demo-video-capture/03-video-docs`, `docs:`: `docs/readme-video/SPEC.md`, the minimal content edits,
   `.agents/docs-media.md`, root `AGENTS.md`, `docs/gui-agent.md`, the ownership doc, and the TODO.md entry removal.
   Record the video with the finished stack and hand it over in the report.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md` (its entries for the GUI agent tooling and the
docs shots), choose the smallest set of checks that would expose a regression, and report checks run, reused and
skipped. Expect `cargo fmt` check, clippy on `farhelm-gui` with the tooling feature, the overlay's Rust tests and the
tooling's Python tests through `python3 scripts/record-test-run.py`, `python3 scripts/check-agents-md.py`, and the live
recording run.

## Fallbacks

- If the bridge's recorder cannot keep up at 1920×1080 on this machine at any usable frame rate, record at the rate it
  sustains and say so in the report; patching the vendored recorder for timestamps or pause is a departure: run the
  scope reassessment.
- If an overlay action needs the app to look up an element's bounds itself every frame (GPUI offers no public bounds by
  id), keep harness-measured boxes and accept drift the stills show, rather than adding that machinery.
- If a beat cannot be made to work on the desktop app at all, drop that beat, keep the rest, and say so in the report.

Decisions to log: the overlay's actions and feature gate, the frame rate and why, how segments are timed and joined,
every content edit to the design files and why, every test changed and why, and every review finding you decided not to
follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-demo-video-capture-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-demo-video-capture-watchdog.json` beside it. Resolve both to
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/demo-video-capture/<nn>-<short-name>`, as
  the outline names them.
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
