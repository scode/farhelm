# Launcher fields keep invisible characters when edited

Written against main at 04631d86 on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Clone: keep invisible characters when a copied folder or name is edited.** Since gpui-last-mile-fixes (#4059), Clone
> shows invisible characters in the copied folder and session name spelled out (`<U+200B>`), using the rule the command
> fields already had. The first keystroke in the field turns those spellings into literal text: type a letter and delete
> it, and the field looks unchanged, but the launch uses a folder path containing the eight characters `<U+200B>`, or a
> title containing `<U+202E>`. A wrong folder path is hard to spot. Turn spellings that are still in the edited text
> back into the characters they stand for (about 10 to 15 lines in
> `crates/gui/farhelm-gui/src/launcher/view/shown_text.rs`), for the folder and name and likely the command fields too;
> the test `clone_shows_its_copied_folder_and_name_escaped_and_editable` pins today's behavior. In the same area: in
> Restart with and Replace with, the suspended-host refusal (#4056) says "Resume it in Hosts, then launch" while the
> button says Restart or Replace; word the ending per mode. Also from the same plan's re-review, test cleanup worth a
> few dozen lines: cases added under old test names (`a_retired_container_setup_is_refused`,
> `a_row_names_a_custom_model_escaped`, `local_home_moves_to_the_local_host`, and
> `a_suspend_in_flight_leaves_new_available`, which drives the whole launcher UI to build a draft a helper already
> builds), and copy-pasted neighbours in `launch_tests.rs` and `environment_recents_tests.rs`. Found by a skeptical
> re-review of GPT-6.2 Sol plans; recorded by the maintainer on 2026-11-18.

### Acceptance criteria

1. **Escape-back, every editable spelled field.** In every launcher field that shows invisible characters spelled out
   (`<U+200B>`) while it can be edited, an edit never turns a spelling into literal text. Those fields are Clone's
   folder and session name (a managed Clone's numbered session name included), the command line and resume command in
   New, Clone and Replace with, Restart with's command fields once Clear agent command has made them editable, and
   Custom flags in every mode. Typing a letter and deleting it leaves the value exactly as it was, invisible characters
   included, and the field keeps showing them spelled out while the value holds any. The launch, the saved template and
   every other use of the value get the real characters. Decoding an edit and choosing how a field displays use one
   shared rule for which fields this covers, so no field decodes an edit and then shows the result raw (which would
   bring back the very bug this fixes). This restores SPEC_impl.md's "Host-supplied text" rule that "an escaped spelling
   never becomes a saved value".
2. **The decoder matches the app's own spelling.** Only `<U+` followed by the hex digits `spelled()` writes (upper-case,
   4 to 6, with no extra leading zero, so `<U+0200B>` stays literal) and `>`, whose character is one the spelling rule
   hides (`is_presentation_unsafe`), turns back into that character. Any other text stays as typed (`<U+0041>` stays
   literal). The decoder lives in `crates/gui/farhelm-gui/src/launcher/view/shown_text.rs` beside `spelled()`, not in
   `crates/farhelm-proto/src/text.rs`, whose owner allows only uses from area code. The round-trip test with `spelled()`
   uses inputs without literal notation in them (see criterion 3).
3. **Typing a spelling yourself.** If the user types `<U+200B>` into one of these fields, it becomes the real invisible
   character. The maintainer accepted this on 2026-11-18. A consequence nobody stated on the planning page: a copied
   value that holds the literal text `<U+200B>` spells out the same as one holding the real character, so any edit to
   that field turns the literal text into the character, and the field looks unchanged. Say so in the fragment's context
   and the report. SPEC.md's "Command launch" says the command "reaches Bash unchanged except for Farhelm's
   placeholders"; add a sentence there saying that in the launcher's fields a spelled-out invisible character stands for
   that character. Check that existing validation still refuses what it refused before when a typed spelling decodes to
   NUL, a newline or ESC, and log what you found.
4. **Fields not affected stay as they are:** the wrapper prefix and the initial prompt (read-only with a Clear button
   while they hold an invisible character), the model field, and Restart with's stored command while it is read-only.
5. **Suspended-host refusal matches the button.** When a launch is refused because its host is suspended or stopped, the
   refusal's ending follows the launcher's mode:
   - New, Clone and quick launch: "… is suspended. Resume it in Hosts, then launch." and "… is stopped. Resume it in
     Hosts, then launch." (unchanged);
   - Restart with: "… is suspended. Resume it in Hosts, then restart." and "… is stopped. Resume it in Hosts, then
     restart." (also while the button reads "Stop and restart");
   - Replace with: "… is suspended. Resume it in Hosts, then replace." and "… is stopped. Resume it in Hosts, then
     replace."

   The state clause ("is suspended." or "is stopped.") stays in the provider's UI module (`environment_ui.rs`,
   `ProviderUi::suspended_launch_advice`); the mode's ending comes from an exhaustive match on the launcher's mode in
   `launcher/request.rs`, with `Refusal::ResumeHost` carrying both parts. That follows root `AGENTS.md`, "Code rules":
   provider wording lives in the provider's UI module, and shared code compares no providers.
6. **Test tidying, the four named tests only:** split `a_retired_container_setup_is_refused` (`recents_view_tests.rs`),
   which also checks that taking a usable setup clears the refusal; split the half of
   `a_row_names_a_custom_model_escaped` (`recents_view_tests.rs`) that checks taking the row fills the field escaped and
   sends the raw id; rename `local_home_moves_to_the_local_host` (`folder_list_tests.rs`), which also checks that Local
   home counts as a hand pick for Save as template; and make `a_suspend_in_flight_leaves_new_available`
   (`session_list/environment_tests.rs`) build its ready draft by editing the draft directly after clicking New (for
   example with `folder_default_tests::edit_draft` made `pub(crate)`), which keeps more of what the test proves than
   swapping in a ready-made command draft, instead of clicking through every launcher field. The entry's unnamed
   "copy-pasted neighbours" are out of scope.
7. **Tests for the behavior:** a decoder unit test (the round trip with `spelled()`, and the texts that must stay
   literal); the three tests that pin today's behavior change to the new one
   (`launcher::entry_tests::clone_shows_its_copied_folder_and_name_escaped_and_editable`,
   `launcher::view_tests::copied_command_text_holding_invisible_characters_shows_escaped_and_editable`, and the flags
   half of `view_tests::agent_launch_text_holding_invisible_characters_shows_escaped`); and Restart with and Replace
   with cases beside `request_tests::a_suspended_environment_names_the_resume_needed_before_launch`.
8. `docs/gpui/parity/launcher.md`'s rows for Clone's spelling, the command and Custom flags fields, and the suspended
   refusal say what is true afterwards.
9. Changelog fragments per root `AGENTS.md`, "Changelog fragments", and the last PR removes this plan's TODO.md entry.

## Decisions already made

Settled with the maintainer on the 2026-11-18 planning page:

- **One rule for every editable spelled field**, not only Clone's folder and name. In a command the old behavior is
  worse than in a folder: `<` and `>` are shell redirections, so a launch could read from or overwrite a file.
- **A typed spelling becomes the real character** (criterion 3). Rejected: escaping typed `<U+` as the user types, which
  rewrites the field mid-typing.
- **The refusal matches the button** (criterion 5). The maintainer asked for the background first: the refusal is the
  launcher's message when Restart with or Replace with is pressed after the session's sandbox or container got suspended
  while the launcher was open. Rejected: "Resume it in Hosts first." for every mode.
- **Only the four named tests are tidied.**
- **No release note for this plan** (see Release note); each PR still leaves its changelog fragment.
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **Builds and tests on the executing machine,** not the boat sandboxes.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and Decisions above.

**Binding repository constraints:** SPEC_impl.md, "Host-supplied text"; SPEC.md's host text and "Command launch"
paragraphs; root `AGENTS.md`, "Code rules", "Changelog fragments", "Conventional Commits", "Tests and validation";
`.agents/test-authoring.md`.

**Planner proposals** (from reading the code at 04631d86; follow the code where it differs, logged as a DECISION):

- Today the field subscription in `launcher/view.rs` (around line 815) calls `edited()` (around line 921) with the
  field's visible text and passes it straight to the draft (`edit_folder`, `edit_name`, `edit_command`,
  `edit_command_resume`, `edit_permission_flags`). `shown()`, `folder()`, `name()` and `as_opened()` in `shown_text.rs`
  show the spelling only while the field still holds the value the launcher opened with (`Opened::of`).
- **Decoding alone is not enough.** If only `edited()` decodes, the edited value no longer matches the opened one, the
  display rule shows it raw, and `sync_fields` swaps the visible spelling for an invisible character. So the display
  rule changes too: an editable spelled field shows `spelled(value)` whenever the value holds a hidden character, and
  every edit decodes. Clone's folder and name stay limited to Clone mode. The opened-value comparison remains only where
  a stored value is read-only, so `Opened` shrinks. `sync_fields` already re-spells on every edit for fields judged on
  what they hold (`Input::shown_through_text_rules`), but that rule as it stands covers the command fields in every
  mode, Restart with included, and leaves out a managed Clone's numbered name; criterion 1's single rule has to get both
  right. Rewrite `shown_text.rs`'s module docs to match.
- Expected size: PR 1 about 40 to 60 production lines and 60 to 100 test lines; PR 2 about 20 production lines and 60 to
  90 test lines.

## Outline

1. `plan/clone-keeps-invisible-characters/01-escape-back`: criteria 1 to 4, 7 (the decoder and pinning tests), 8 (the
   field rows). `fix:`, with a fragment.
2. `plan/clone-keeps-invisible-characters/02-refusal-wording`: criteria 5, 6, 7 (the request tests), 8 (the refusal
   row). `fix:`, with a fragment. Removes this plan's TODO.md entry.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
`farhelm-gui`, the GUI crate's launcher, templates and session-list environment unit tests through
`python3 scripts/record-test-run.py`, and `python3 releasing/check-changelog.py format`. No golden run unless something
drawn changes.

## Fallbacks

- If another change has moved the launcher's field handling, follow the current code and record the true names in the
  working log.
- If one of the named tests no longer exists or was already split, skip it and log that.
- If the mode's ending cannot be chosen without the provider UI module learning about launcher modes, that is a
  departure: run the scope reassessment.

Decisions to log: how the display rule changed, which fields it covers, every test changed and why, and every review
finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-clone-keeps-invisible-characters-log.md` in the parent directory of the checkout you run in, derived as
that section says, and the watchdog's status file is `farhelm-plan-clone-keeps-invisible-characters-watchdog.json`
beside it. Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the
agent scratch directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests on the machine you run on, as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`. The maintainer chose this on 2026-11-18; do not use the boat sandboxes
(`.agents/boat.md`) for this plan.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (one of its remotes
points at that GitHub repository; if not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill
with the log file's absolute path. If the log exists, read it and resume where the previous session left off,
cross-checking it against reality (the jj graph, bookmarks and open PRs) rather than starting over. If it does not
exist, this is a fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the
resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7) before any work.

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
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/clone-keeps-invisible-characters/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments").
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
