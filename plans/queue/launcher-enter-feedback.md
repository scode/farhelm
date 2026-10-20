# Session launcher: Enter always launches, picks something visible, or says why not

Written against main at be722d41 on 2026-10-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file says which of their sentences to change.

This plan has no dependency on another plan.

## The goal

In the session launcher (New, Clone, Replace with), pressing Enter can end with neither a launch nor any message. The
case that prompted this: Clone a session, type `asdf` into the search box at the top, press Enter, and nothing happens.
Make every such Enter either do something visible or say why it did nothing.

TODO.md entry this plan covers (Near term): "**Say why Enter in the session launcher did nothing.** When pressing Enter
in the session launcher does not launch, for whatever reason, show the reason instead of silently doing nothing. The
case that prompted this: Clone on a session, type `asdf` into the launcher's text field at the top, press Enter, and
nothing happens. The launcher already explains a refused launch when its prerequisites are missing (the Launch button's
tooltip, and a message after an attempt), but that does not cover this. The work is to find every other case like it,
any way an Enter in the launcher can end without a launch or a message, and give each one a reason. For the `asdf` case
specifically, one suggested behavior is to treat bare text as `name:asdf` (what the maintainer meant there); that is
decided when the entry is planned."

SPEC.md currently requires part of today's silence ("Enter on a non-empty query with no result never launches, and
Escape closes the result list without clearing the query, so Enter after Escape does nothing until the box is emptied").
This plan rewrites that sentence.

### Acceptance criteria

**Search box:**

- **Unlabelled text that matches nothing** (`asdf`, a mistyped model name, an unknown label such as `foo:bar`): the
  results list shows one visible row, "Set session name: asdf" (the row `name:asdf` produces today), in the Names group.
  Enter picks it like any row: it sets the session name exactly as accepting `name:asdf` does (making the name the
  user's own, which turns off checkout numbering, and clearing the search box without launching), so a second Enter
  launches. The row appears only when no other result matches; text that matches anything else shows those results and
  no name row. The name is never set invisibly at submit. Build the row in one shared helper applied after all result
  sources are combined, used by both the code that draws the list and the code that handles keys (today the rows are
  assembled twice, and the key-handling copy leaves out `gh:` rows; keep the two in agreement).
- **Live messages in the results area** for every other search-box case that would otherwise show nothing, shown while
  typing, not only after Enter, in the place the results list would be. With nothing typed after them, most labels
  (`host:`, `model:`, `harness:`, `folder:`, `recent:`, `tl:`) already list every choice, so the messages needed are:
  - `name:` with nothing after it: a hint to type a name;
  - a known label whose value matches nothing (`host:missing`, `harness:xyz`, `folder:` or `recent:` with no match): a
    generic "no ‹label› matches" message naming the label;
  - `model:` text that matches no known model: that a custom model id goes in the model field; and a loading message
    while the model list has not arrived;
  - `tl:` while templates are loading, or when none match: loading, or no match;
  - `gh:` text that is not an owner/repo and matches nothing discovered: that it needs owner/repo;
  - on the agent tab before a harness is chosen, text that is exactly an effort or permission word (`high`, `yolo`,
    `guarded` and the other effort words, matched as whole words, not substrings) or uses the `effort:` or `perms:`
    label: "choose a harness first" (or equivalent wording; log it). This message replaces the name row for those words.
    On the command tab those words get no special treatment;
  - text that is only spaces: a short hint (it is treated as no search, and does not launch). An Enter in any of these
    cases does nothing beyond what the visible message already says.
- **The message and the list are one area:** Escape, or a click elsewhere in the launcher, closes it, whether it holds
  results or a message. Enter with the area closed reopens it without picking anything; a second Enter picks the
  highlighted row (or does nothing more than show the message again).
- **Repository field** (managed checkout): an Enter with no suggestion to pick, or with the suggestion list closed, gets
  the same treatment as the search box: a visible message when nothing matches, and a closed list reopens.

**Submit path:**

- A refusal is never dropped silently: today, when the launch target changed while the launch was in flight, the refusal
  is discarded (the branch that only shows the error if `create_target` still equals `target_now`). Show a fixed message
  such as "a launch for an earlier setup was refused" rather than the raw refusal, which keeps the existing rule that
  one host's refusal never shows under another host.
- Custom permission mode with empty or unparsable custom flags gets a reason that names the flags, instead of "this
  saved choice is no longer supported by the current catalog; choose a compatible model or effort". One more condition
  in the existing launch prerequisites (`LaunchPrerequisites`) is enough.
- The broken message string in `create_form.rs` that renders as "…submitted, so nothing was sent…" reads normally.
- While "loading session setup…" shows, Enter on the dialog itself still calls `requestSubmit()` (the dialog's Enter
  handler from `dialog_focus.rs`), so a launch can be attempted before the setup has loaded (confirmed by the planning
  review from the code). Fix it: Enter then does not submit, and the loading line is the visible reason.

**Stay silent on purpose (unchanged):** a held Enter, Enter during input-method composition, the model field's own Enter
rules, the checkout-folder setup field (saving configuration and launching are separate acts), Enter with nothing queued
in an armed quick-launch sequence (SPEC.md specifies that it cancels), and the one keystroke modal isolation swallows
when focus is outside the dialog. Two races (Enter in the one render while the box is disabled for a busy operation, and
Enter on a recent-setup row whose host changed under it) get a reason if an existing message surface makes that cheap;
otherwise log them and say so in the report.

**Also:**

- The message text is built by a pure function of the query, the tab, whether a harness is chosen and what has loaded,
  with unit tests covering the grammar cases above; Enter itself needs only three outcomes (pick the highlighted row,
  reopen the area, do nothing), decided by a small pure function in the style of the model field's `model_enter_target`.
  Browser tests cover the wiring: the `asdf` case after Clone (Enter sets the name, a second Enter launches), one label
  message, the Escape-then-Enter reopen, the repository field, and the loading-state Enter. The rest of the grammar is
  unit-tested, which is cheaper.
- SPEC.md (the launcher's search grammar and Enter rules, around lines 608-626, and the sentences "Unlabelled search
  retains ordinary matching…" and "Accepting a name or host action clears search without launching", around 1066-1071)
  and the docs page `website/src/content/docs/docs/using/start-a-session.mdx` (the search prefixes section) describe the
  new behavior.
- A changelog fragment (`kind: fixed` or `changed`; log which).
- The last code PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-20), from the planning page:**

- `asdf`: "Offer a visible "Set session name: asdf" row only when nothing else matches", chosen over a message with no
  automatic name and over always offering a name row first. Recommended because Enter then picks a row the user can see,
  and partial words (`cla` for Claude) keep working.
- Messages: "Live, in the results area while typing", chosen over messages only after Enter.
- Closed list with results: "Reopen the list; a second Enter picks", chosen over a message and over keeping today's
  silence.
- Also fix the repository field, the dropped refusal, the misleading custom-flags reason, and the broken string: "All of
  them".
- Review gate and mode are in the run instructions below.

**Binding repository constraints:**

- SPEC.md, the launcher's search grammar and Enter rules (the paragraph with "`name:foo` applies the entire value as the
  session name", "Enter on a non-empty query with no result never launches…", "search box, model field, YOLO
  confirmation, and save-as-template panel keep their own Enter rules", "A held Enter never launches"); the sentences on
  unlabelled search, unknown labels and colons in model IDs, and accepting a name or host action; the rule that
  accepting `name:` makes the name the user's own and disables numbering; the sequence rule that Enter with nothing
  queued cancels.
- The code-level rule in `launch_composer.rs` that "the query's value is never a hidden title or host on submit"
  (`name_host_search_results`' doc comment): keep it true; the fallback is a visible row.
- Root `AGENTS.md`, "Changelog fragments" and "Tests and validation"; `.agents/test-authoring.md`; `website/AGENTS.md`
  before editing the docs page; `.agents/docs-media.md` before touching `e2e/docs-shots/` (the start-a-session search
  shots may change).

**Planner proposals (checked by a fresh-context planning review):** the outline below. A planner proposal is not a
requirement: a smaller design that meets the same acceptance criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main (verify them):

- The launcher is `CreateSessionForm` in `crates/farhelm-ui/src/list/create_form.rs`; search grammar and results are in
  `crates/farhelm-ui/src/launch_composer.rs` (`scoped_query`, `name_host_search_results`, `template_search_results`,
  `search_results`, `grouped_search_results`, `default_search_index`); shared controls in
  `crates/farhelm-ui/src/launch_controls.rs`.
- The search box's Enter handler (around `create_form.rs` 6201-6227) suppresses the browser submit for any non-empty
  query and accepts the highlighted row only if the list is open and a row exists; the list is not drawn at all when
  there are no groups (around 6233). Escape closes the list (around 6168-6172), and the form's `onclick` closes it for
  any click outside the search wrapper (around 5040-5047).
- Existing message surfaces: the reason beside Launch (`.launch-composer-refusal`, built from
  `LaunchPrerequisites::refusal()`), and the line under the search box (`.launch-composer-template-error`) used for
  template and sequence refusals. A "no matches" line has a precedent in the quick switcher ("no templates match").
- The repository field's Enter is always suppressed (around 6598-6604); the note under the field can carry its message.
  The dropped refusal is the async failure branch around 5691. The custom-flags case goes through
  `check_permission_flags` (defined in `farhelm_proto`, called from `launch_composer.rs`) and `structured_choice_error`
  in `create_form.rs`. The broken string is around `create_form.rs` 5182.
- Tests: `e2e/tests/launcher-enter.spec.ts` (the visible-refusal suite and its `expectReasonByLaunch` helper),
  `e2e/tests/composer-word-search.spec.ts` (grammar; its `command`-has-no-result check at around 284-288 changes, since
  `command` now offers a name row), `e2e/tests/clone.spec.ts`, `github-checkout-composer`, `tooltip-coverage`; Rust
  tests in `launch_composer.rs`. `search_never_treats_its_text_as_a_command` tests the raw search results, so it should
  not need to change if the name row is added after the results are combined.
- The docs screenshots probably do not change; check per `.agents/docs-media.md` only if the search shots show an
  affected case.

Proposed shape (planner proposal), as a short stack:

1. `fix`: the message and Enter functions, the name-row fallback, the live messages, the closed-area reopen, tests, SPEC
   and docs, fragment.
2. `fix`: the repository field, the dropped refusal, the custom-flags reason, the broken string, the loading-state
   check, tests; remove the TODO.md entry. Fold this into the first PR if it is small; log the choice.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the UI crate's unit tests, and the launcher browser specs named above
through the recorder on both engines; the test-sleep checker for changed browser tests; the website checks
`website/AGENTS.md` names for the docs edit; `python3 releasing/check-changelog.py format`.

## Fallbacks

No complexity gate. Examples of departures that need the scope reassessment: splitting the launcher component as part of
this work (a separate TODO entry proposes that), a new message surface beyond the results area and the existing two, or
changing how `name:` itself behaves.

Decisions to log: each message's wording, how "nothing else matches" is decided (and that it depends on live folder and
recent-setup history, so the same text can offer a name row one day and a folder the next), how the dropped refusal is
shown, and the fragment's kind.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-launcher-enter-feedback-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: they keep the model and
effort this file demands for them. The user also forbids invoking, loading or following `scode-galaxy-brain` or
`scode-model-routing` at any point; if galaxy-brain is already active when you start, this file is the user's express
instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The scope reassessment review runs
at your own model and effort. The sub-agents `plans/AGENTS.md` (Sub-agents this file requires) requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) run as that file says.

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
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/launcher-enter-feedback/<nn>-<short-name>` (for example `plan/launcher-enter-feedback/01-search-enter`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

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
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
