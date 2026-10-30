# UI: type template names in a quick-launch sequence

Written against main at 462b0268 on 2026-10-29. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> - **Emacs-style quick launch by typed template names.** Add a quick launch that also supports typing template names
>   separated by spaces, so `cmd+x myproject codex Enter` launches my project in Codex. Faster than `tl:...` in the
>   launcher.

Cmd+X in the Mac desktop app, or Ctrl+Shift+X everywhere else, already starts a quick-launch sequence (SPEC.md,
"Creation", the bullet beginning "Quick-launch sequences compose templates"): number keys queue templates, Enter applies
them in order (later edits winning) and launches through quick launch's direct path, and any other key cancels the
sequence and reaches its normal target. This plan extends that sequence so template names can be typed as well. It adds
no new chord.

### What changes

While a sequence is armed:

- **Typing a name.** A printable character (including Shift for capitals) that is not a sequence key starts or extends
  the word in progress instead of cancelling. Space (with or without Shift) ends the word and queues it as typed; Space
  with no word in progress does nothing.
- **Number keys** keep their meaning when no word is in progress: a bare digit or the digit chord queues that key's
  template. A digit typed while a word is in progress is part of the word. So a name that starts with a digit cannot be
  typed (only its number key, if it has one, reaches it); the spec and docs say so. The digit chord or the ⚡ chord
  pressed while a word is in progress first queues the word, then the chord's template.
- **Tab** completes the word in progress when it identifies exactly one template by the matching rules below (an exact
  match wins, including over longer names it is the start of), queuing that template's full name as one entry, spaces
  included. Otherwise, or while the catalog read below has not returned or has failed, Tab does nothing.
- **Backspace** removes the last character of the word in progress, and otherwise does nothing; it never cancels and
  never removes queued entries.
- **Enter** queues a word in progress, then launches (on the page) or applies to the draft (in the session launcher) as
  today. Typed words are resolved at that moment against the templates the sequence already reads fresh at Enter.
  Shift+Enter keeps today's behavior.
- **Matching** a typed word: an exact case-sensitive name match first; then a unique case-insensitive exact match; then
  a unique case-insensitive prefix (Unicode case folding, as Rust's `to_lowercase` gives). Anything else (no match, or
  several) is refused: the refusal names the word and, when ambiguous, its candidates, and nothing launches or applies.
  On the page the refusal opens New with that error, the way a missing template already does (when a sequence fails both
  on an unbound number key and on a word, the word's refusal wins); in the launcher it shows beneath the search box
  without changing the draft.
- Keys that are none of these (arrows, other chords, function keys, Shift+Enter) still cancel and reach their normal
  target. While armed, typed characters never reach the terminal's program.
- **The pill and its candidate list.** The amber status pill shows queued entries as chips (typed words as typed, or as
  Tab completed them) and the word in progress, and its hint names typing, Tab, Enter and Escape. Below it, while a word
  is in progress, a candidate list shows the template names it currently matches (exact matches first, then prefix
  matches, a few at most); non-matching names are absent. Like the pill, it takes no pointer input and never takes
  focus. The candidates and Tab use a template catalog read when the prefix starts, a new read alongside the bindings
  read that already happens there; that read never decides what Enter launches.
- **Template names.** A new or renamed template name may no longer contain whitespace (`char::is_whitespace`, so a Mac
  no-break space counts). The rule is enforced in exactly three places: the Templates editor's save when the name
  differs from the one it was opened with, Save as template, and the helm's `farhelm agent template create` branch
  (`agent template edit` cannot rename). The helm's template store and its `PUT` stay shape-only, and the shared
  name-collision helpers stay as they are, because Templates' Undo re-creates a deleted template through them; a check
  there would block undoing the delete of a spaced template. Existing names with whitespace stay valid: they keep
  loading, applying, launching, saving unchanged and deleting with undo, and never show as failing current save rules
  (which would turn their Delete into "Delete permanently").
- **Duplicate's suggested name** becomes `<name>-copy`, then `<name>-copy-2`, and so on, with any whitespace in a spaced
  source's name replaced by `-` in the suggestion.

A spaced name is reachable in a sequence through Tab when the text before its first space identifies it uniquely; with
both `my project` and `myproject` stored, `my project` is reachable only through its number key or the launcher. The
spec and docs state that residual. No quoting or candidate cycling is added.

### What does not change

The prefix chord and where it is admitted (terminal yes; text fields outside the launcher keep Cut; launcher fields only
without a selection), the single-chord ⚡ and number shortcuts, Cmd+J's template list, `tl:` search, template stacking
semantics, the pill's position, look and no-timeout rule, the existing refusal, incomplete-setup, YOLO and unbound-key
fallbacks, "Enter with nothing queued opens nothing", and the "Name sessions on quick launch" rename. Bindings are still
read when the prefix starts and templates at Enter.

### Acceptance criteria

- `⌘X` (or `Ctrl+Shift+X`) `myproject codex Enter`, with templates named `myproject` and `codex`, launches exactly what
  `myproject` then `codex` applied by hand would, on the page, even when typed faster than the prefix-start catalog read
  returns; in the open session launcher the same keys apply them to the draft without launching.
- The matching order above holds, including `Codex` and `codex` both stored; an unknown or ambiguous word is refused at
  Enter with a message naming it (and its candidates), and nothing is launched or applied.
- Tab completes a uniquely identified word (including to a name with spaces) to one queued entry and does nothing
  otherwise.
- Typed words and number keys mix in one sequence in the order entered, with the digit and ⚡ chord rules above.
- Backspace edits only the word in progress; Escape and other non-sequence keys cancel as today; while armed, typed
  characters never reach the terminal's program.
- The pill shows the queued chips and the word in progress, and the candidate list follows the word, without taking
  pointer input or focus.
- Creating (editor, Save as template, agent `template create`) or renaming in the editor to a name with whitespace is
  refused with a clear message; duplicating suggests the `-copy` names above; an existing template whose name has a
  space still loads, applies, saves unchanged, deletes with undo, and is reached by Tab when uniquely identified.
- If the `shortcuts-overview` plan's keyboard shortcuts dialog is already on main when you run, its quick-launch
  sequence section lists typing names, Space, Tab and Backspace.
- SPEC.md ("Creation"'s sequence bullet and template-name rules, "Desktop window chrome"'s Cmd+X paragraph if needed)
  and SPEC_impl.md describe the new behavior, the residuals above included; the docs' "Combine templates from the
  keyboard" section in `website/src/content/docs/docs/using/launch-templates.mdx` explains typing names, Tab and the
  no-spaces rule, and no longer says any other key goes straight back to the terminal.
- Tests: Rust tests for word resolution, the name rule (new versus existing and Undo) and the duplicate suffix; JS unit
  tests for the armed-sequence key classification; browser tests in `e2e/tests/quick-launch.spec.ts` and
  `e2e/tests/launcher-template-shortcuts.spec.ts` for the acceptance cases above. Existing tests that assert the
  behavior this plan reverses are rewritten, not weakened: the browser test "launch sequence cancellation leaves
  ordinary terminal input and modals in control" (it types text after arming; keep it proving that a non-sequence key
  such as ArrowUp or a Ctrl chord cancels and reaches the terminal), the JS tests "unrelated input cancels before
  passing through" and "extra modifiers, keypad digits and IME never queue", the Rust test
  `duplicate_names_are_free_and_fit_the_name_limit`, and the duplicate case in `e2e/tests/templates.spec.ts` that
  expects `` `${source} copy` ``.
- Changelog: the first PR adds a fragment for the name rule (`kind: changed`), and the second rewrites that same
  fragment into one combined `kind: added` entry for typing names, mentioning that new template names can no longer
  contain spaces.
- The TODO.md entry quoted above is removed.

## Decisions already made

Settled with the maintainer on 2026-10-29 on a planning page with mockups:

- **Extend the existing Cmd+X sequence**, rather than a new chord (stated on the page; the maintainer did not object).
- **Matching:** case-insensitive exact name, else a unique prefix; unknown or ambiguous words are refused at Enter.
  Chosen over exact names only and over unique substring matching. (The case-sensitive-first tie-break above is a
  planner refinement for names that differ only in case.)
- **Spaces in names:** forbid spaces in new template names; existing ones are reachable only by Tab. Chosen over Tab
  completion alone with spaces still allowed, and over greedy longest-match parsing.
- **Design B, the pill plus a candidate list**, chosen over completion shown inline in the pill alone. The mockup showed
  the existing amber pill reading "Launch sequence: [myproject] co · Tab complete · Enter launch · Esc cancel", with the
  queued name as a chip and the partial word in monospace, and below it a small bordered box listing "codex ·
  codex-yolo" (the mockup's greyed "(no match)" entry was illustrative; non-matching names are absent). Match the pill's
  existing styling rather than the mockup's exact look.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md, "Creation" (launch templates, quick launch, number shortcuts, quick-launch
sequences) and "Desktop window chrome" (the Cmd+X paragraph); SPEC_impl.md's account of the sequence state machine,
template shortcuts and the relay buttons; `check_template_shape` in `crates/farhelm-proto/src/launcher.rs`, which the UI
also uses to decide that a stored template "fails current save rules" (`crates/farhelm-ui/src/list/templates.rs`), so
the no-whitespace rule must not be added there; root `AGENTS.md`, "Code rules", "Tests and validation", "Conventional
Commits", "Changelog fragments"; `website/AGENTS.md` for the docs edit; `.agents/test-authoring.md`;
`.agents/validation.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION), refined by a fresh-context
planning review against the code:

- **Who owns the word.** Today's relays are hidden buttons whose clicks carry no payload, and in the desktop build JS
  cannot read Rust's state back synchronously. Rust owns the word in progress, the queue, Tab and Backspace, and decides
  digit-versus-word itself. JS keeps only "armed", consumes printable keys, Space, Tab and Backspace while armed, and
  forwards each one in order through a single payload-carrying relay (for example a hidden input whose `oninput` Rust
  reads), sending a digit as both the number key and the typed character. JS still clicks the launch relay at every
  Enter, so each Enter press pairs with exactly one worker entry as the FIFO comment in
  `crates/farhelm-ui/assets/template-shortcuts.js` requires; an empty batch already drains harmlessly, which keeps
  "Enter with nothing queued opens nothing".
- **Resolution.** A typed word travels as its own reference variant (for example `TemplateReference::Typed(String)` in
  `crates/farhelm-ui/src/launch_sequence.rs`), resolved in `resolve_template_references` or a sibling against the
  templates the dispatcher already reads at Enter (`crates/farhelm-ui/src/list/view.rs`), with a refusal that names the
  word and its candidates and is routed the way the dispatcher already routes refusals. `TemplateReference::Name` keeps
  its exact-name meaning and its "no longer available" refusal.
- **Pill.** The pill and candidate list stay in `LaunchSequenceIndicator`.
- **Two PRs:** the name rule and duplicate suffix first, then the typed sequence.

## Outline

1. **Name rule.** The no-whitespace check in the editor's save (when the name changed), Save as template, and the helm's
   agent `template create` branch; the `-copy` duplicate suggestion; Rust tests for refusals, an unchanged spaced name
   saving, and Undo of a deleted spaced template. Changelog fragment (`kind: changed`).
2. **Key forwarding.** Extend the armed-sequence classifier in `template-shortcuts.js` to consume printable keys, Space,
   Tab and Backspace and forward them through the payload relay; JS unit tests; rewrite the two JS tests named above.
3. **Word state and resolution.** The word, Tab and Backspace in Rust; the typed reference variant and its resolution
   and refusal at Enter; the prefix-start catalog read for candidates and Tab; Rust tests.
4. **Pill and candidates.** Queued chips, the word in progress, the candidate list and the hint text.
5. **Spec and docs.** SPEC.md and SPEC_impl.md; `launch-templates.mdx` (incidental docs edit in an unattended run: no
   preview server, name the changed page in the report). If the shortcuts dialog exists on main, update its sequence
   section.
6. **Tests.** Browser cases for launch on the page (including typed faster than the catalog read), apply in the
   launcher, matching order, ambiguity, unknown word, Tab with a spaced name, digits and chords mixed with words,
   Backspace, and typed characters not reaching the terminal; rewrite the existing tests named above.
7. **Bookkeeping.** Rewrite the fragment into the combined `kind: added` entry; remove the TODO.md entry in the last PR.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`, clippy for the changed crates with
`--all-targets -- -D warnings`, the changed crates' tests, the JS unit tests, `dprint check` on changed Markdown,
`python3 releasing/check-changelog.py format`, and, through `python3 scripts/record-test-run.py`,
`quick-launch.spec.ts`, `launcher-template-shortcuts.spec.ts`, `template-list-shortcut.spec.ts` and `templates.spec.ts`
on Chromium and WebKit. Run `python -B scripts/check-test-sleeps.py` when browser tests change.

## Fallbacks

Departures that need the scope reassessment: a new chord instead of extending Cmd+X; fuzzy or substring matching;
quoting syntax or candidate cycling for spaced names; renaming or migrating existing templates with spaces; making the
candidate list clickable or focusable; letting Backspace remove queued entries; changing Cmd+J or `tl:`; moving word
ownership into JS.

Decisions to log: the payload relay's shape, how the prefix-start catalog read is made, how many candidates the list
shows, where each name-rule check lives, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-typed-quick-launch-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the code reviewer below, for instance), `$scode-harness-shellout` supplies the
launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/typed-quick-launch/<nn>-<short-name>`
  (for example `plan/typed-quick-launch/01-no-space-names` and `plan/typed-quick-launch/02-typed-sequence`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments").
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Code review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
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
findings file in the scratch directory, and the acceptance criteria (this file's goal, what changes and what does not,
acceptance criteria, decisions and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name, a class name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its code review gate, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one
plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
