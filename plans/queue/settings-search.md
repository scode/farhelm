# Settings search

Written against main at 63d8ecd5 on 2026-11-19. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Settings search.** support search in settings dialog.

The maintainer's note from the planning picker, verbatim:

> basic search across all settings, please make realistic mocks with suggested looks. make sure mocks are not tiny in a
> little box i can't scroll. i need to be able to see what it looks like

The maintainer chose the design on full-size mockups of the real dialog. Described in words, since the mockups live
outside the repository:

- A search field (placeholder "Search", no icon, no clear button) sits at the top of the Settings dialog's section list
  (the left column, 168 px wide), above the section buttons, full column width.
- While the field holds text, the section list narrows to the sections with at least one match, each showing its match
  count at the right edge of its button; and the right-hand pane is replaced by the results: a heading such as "4
  settings match “update”", a one-line hint, then the matches grouped under each section's name (a small capitalized
  section heading), each result a row with the setting's label over a one-line excerpt of its help, the matched text
  marked in the accent at bold weight, the first result selected. Up and Down move the selection from the field; Enter
  or a click opens the result.
- Opening a result shows its section with that setting focused and scrolled into view. A section-name match lists every
  entry of that section.
- Clearing the field restores the full section list and the section that was shown.

### Acceptance criteria

1. The search covers every section, including sections not yet shown in this dialog, by section name, setting label,
   hover help and help text, plus a short curated synonym list (for example "dark mode", "theme" and "color" find
   Appearance; "sound", "bell", "push" and "phone" find Notifications; "token", "secret" and "api key" find Credentials
   and Tensorlake's key; "update" and "upgrade" find Hosts and App; "podman" finds Containers; "links", "browser" and
   "tunnel" find Terminal; "clone" and "git" find Checkouts; "trust" finds Agent trust). Matching is per query word,
   case-insensitive, substring; every word must match somewhere in the entry. Names the user created (credentials,
   recipes, containers) are not searched.
2. Results are grouped by section in the dialog's section order, and within a section in the order the section draws its
   settings. No ranking beyond that.
3. Opening a result focuses that setting's control for every switch and field. The color theme and terminal web-link
   choices (radio groups) and entries with no control (a warning, a help paragraph) open their section at its top
   instead (the maintainer agreed to that on the planning page).
4. Opening Settings still focuses the first switch of Sessions (SPEC.md). Cmd+F (macOS) / Ctrl+F (Linux) anywhere in
   Settings moves to the search field, from a switch, a button or a text field, except that on Linux a text field may
   keep Ctrl+F for itself (the queued `emacs-text-field-keys` plan makes Ctrl+F move the caret there).
5. Escape in a non-empty search field clears it; Escape in an empty field, and the header × and footer Close, close
   Settings as today.
6. SPEC.md's Settings paragraph and the Settings docs page (`website/src/content/docs/docs/using/settings.md`) gain a
   sentence or two describing search.

## Decisions already made

Settled with the maintainer on the 2026-11-19 planning pages:

- **Results shape:** the section list narrows to matches with counts, and the pane lists every match (chosen over
  filtering inside each section and over a flat list with the section list unchanged).
- **Field placement:** the top of the section list (over a full-width bar under the header).
- **Focus on open:** unchanged (first switch); Cmd+F / Ctrl+F reaches the field.
- **Escape:** clears a non-empty search first, then closes.
- **Matching:** section names, labels, help text and a short synonym list.
- **Radio groups and control-less entries:** open their section rather than focus a choice.
- **Review gate:** a single fresh-context gpt-6.2-sol agent at high effort with the general review charter.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Release note:** the entry under `## Release note` below, as the maintainer accepted it.
- **Ordering:** runs after `docs-shot-desktop-fixes.md`, which changes the Settings → Hosts goldens that this plan
  re-captures along with every other Settings golden.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the picker note, and the picks above.

**Binding repository constraints:**

- SPEC.md's Settings description ("Session list" section, around the paragraph on the Settings dialog): the gear opens
  Sessions with its first switch focused; Tab rules; section switching keeps state; Escape and close return focus to the
  gear.
- The root key context must not bind Ctrl+F: on Linux it would take Ctrl+F from a focused terminal's program, and the
  keymap walk test in `crates/gui/farhelm-gui/src/actions_tests.rs` fails any non-app binding that resolves in a focused
  terminal.
- Clippy runs with `-D warnings` and `farhelm-gui` has no dead-code allowance, so no PR may land code nothing uses yet.
- `.agents/validation.md` for GUI changes and goldens; root `AGENTS.md` "Conventional Commits", "Changelog fragments",
  "Tests and validation"; `.agents/test-authoring.md`; `website/AGENTS.md` before editing the docs page (an incidental
  docs edit starts no preview server; name the changed page in the report).

**Planner proposals** (verified against the code and reviewed by an independent planning reviewer):

- **A static catalog of searchable entries**, because building hidden sections to read their text would send helm reads
  (and, for Containers, a check of every container host) on search. The `Choice` switches and Agent trust's agents come
  from iterating `Choice::ALL` and `TrustAgent::ALL` with their existing `label()`/`tooltip()`/`help()` (visibility
  widened as needed) plus a new exhaustive `Choice::section()`, so coverage holds by construction; the rest are listed
  by hand, each referring to the section modules' string constants rather than copying text. Strings a section spells
  inline (the ntfy form does, in `settings/ntfy.rs`) move into constants first. A test checks that every catalog entry's
  target element is drawn when its section is shown.
- **Reuse the quick switcher's result rows** (`components/option_row.rs`'s `OptionRow` and `option_heading`, with
  `TextInput::on_vertical` for Up and Down); Settings already keeps focus on an empty click, which `OptionRow` needs. Do
  not reuse the quick switcher's tiered subsequence matcher (over paragraph-long help it matches almost everything), and
  do not reuse or widen `host_text::marked` (it exists for host-supplied text; Settings' words are the app's own): mark
  matches with `StyledText` highlights directly. Whether to share the mark style (accent at weight 600, or `on_fill` on
  the selected row, inline in `session_list/quick_switcher.rs`) or its `find` helper is a close call for you; log it.
- **One binding for Cmd+F / Ctrl+F**, in a new key context on the Settings dialog, with one handler. GPUI tries matching
  bindings in precedence order until one does not propagate, and GPUI Kit's search action propagates from a field that
  is not searchable (all of Farhelm's), so the dialog's binding is reached from a field too; nothing needs to handle
  Kit's search action. Test it from a focused switch (on the Linux test host a later Ctrl+F binding in text fields would
  otherwise break a from-a-field test).
- **Escape-clears-first lives on the search field**, not in the dialog's dismissal hook (`SettingsDialog::dismissal`),
  which the header × and the footer Close also ask; putting it there would make Close clear the search.
- **Jump and focus:** switches with dialog-held handles focus at once (`focus::focus_in_layer` scrolls into view).
  Sections whose controls exist only after a read reuse Agent trust's existing wait (`Handover`, `trust_known`):
  generalize its `agent: TrustAgent` to a catalog target so the trust hint and search share it, and give each child view
  that needs it a small "my read answered, here is the handle" hook. Fall back to opening the section where that is not
  cheap, and list those entries in the report.

## Outline

Two PRs, split by behavior so each is used when it lands:

1. `plan/settings-search/01-search` (`feat:`): the catalog and matcher, the search field at the top of the section list,
   the results pane with marked matches, Up/Down/Enter/click, jump and focus, Escape clearing first, Cmd+F / Ctrl+F,
   SPEC.md and docs sentences, the goldens (adding the field changes all 32 existing Settings pictures; add light and
   dark scenes of a search with results, and of no matches), and the changelog fragment (`kind: added`).
2. `plan/settings-search/02-narrow-sections` (`feat:`): the section list narrowing to matching sections with counts; its
   goldens (the search scenes only); edits PR 1's fragment if its wording changes; removes this plan's TODO.md entry.

## Fallbacks

- If publishing updated GUI goldens is blocked because the executing machine lacks push access to the private golden
  storage repository (`scripts/gui-goldens.py` says where), keep the updated captures verified locally and block per
  `plans/.agents/executing.md`, "Executing one plan" (step 10), rather than landing failing goldens.
- If a section's controls cannot be focused through a cheap hook, open the section at its top for those entries, log it,
  and list them in the report.
- If the synonym list grows past about a line per section, keep the most obvious words and log the rest as a follow-up.

Decisions to log: the catalog's shape and which entries are hand-listed, the mark style sharing call, every entry that
falls back to opening its section, and every review finding declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-settings-search-log.md` in the parent directory of the checkout you run in, derived as that section says,
and the watchdog's status file is `farhelm-plan-settings-search-watchdog.json` beside it. Resolve both to absolute paths
before you start; this file does not create either. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/settings-search/<nn>-<short-name>`, as
  the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what that PR
  built, with the kind that fits it (a user-visible fix is `fixed`, not `none`, even when the release note answer is "No
  release note needed."). A later PR in this stack that amends work an earlier PR of this stack built edits that PR's
  fragment instead of adding a second one. Where what got built departs from a release note entry (a renamed control, a
  part left out, a limitation it does not mention), the fragment's context paragraphs say how
  (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- GUI drawing changes update the screenshot goldens in the same PR, as `.agents/validation.md` describes:
  `scripts/check-gui-goldens.py --update`, look at every changed or added picture its report names before committing,
  then `python3 scripts/gui-goldens.py publish` and commit the rewritten manifest lines. A golden change you did not
  expect is a finding to explain or fix, not a picture to accept.
- The last PR of the stack removes this plan's TODO.md entry.

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
meeting the acceptance criteria above, each having passed its review gate, and the last PR removing this plan's TODO.md
entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

Settings now has a search field: type part of a setting's name or description to find it in any section, and pick a
result to go straight to it.
