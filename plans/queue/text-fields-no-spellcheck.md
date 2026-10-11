# Turn off spell checking and autocorrect in every text field

Written against main at 95bb60a7 on 2026-10-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

The TODO.md entry (Near term), "Turn off spell checking and autocorrect in every text field": the template create/edit
dialog still underlines and autocorrects what you type, which is super annoying in fields that hold commands, prompts,
paths and names. Some fields already turn it off (the launcher, rename, and the host fields, among others); make that
true of every text field in the app, the template dialog first.

Acceptance criteria:

- Every text-entry field the app renders (text-like `input`s and `textarea`s, in the web UI and the desktop app) has
  spell checking, autocorrect and autocapitalization off, and browser autocomplete off, with one exception: the feedback
  dialog's contact field keeps its email autofill (a planner proposal; see below).
- That includes the feedback dialog's message box, by the user's explicit decision.
- A browser test fails if a text field in the dialogs it covers is missing any of these attributes, so the next field
  added without them is caught.
- One draft PR (or a short stack, if the test sweep reads better on its own), passing its review gate, with its
  changelog fragment, and removing the TODO.md entry.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-10):** plan this TODO entry.

**The user's decisions (2026-10-10):**

- Off everywhere, including the feedback dialog's message box. (Keeping spell checking there, because it is prose, was
  offered and not chosen.)
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- Root `AGENTS.md`: Conventional Commits; Finishing work; Releases and the changelog; Agent scratch space; Sharing the
  machine with other agents; the rule against tests that modify the test process's own environment variables.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the mechanisms and test shapes in the outline below, checked against main at 95bb60a7. One is
worth calling out because the user did not decide it: the contact field keeps `autocomplete="email"`, since that is the
browser offering the user's own address, not correcting what they type, and turning it off would only make the field
worse. A fresh-context planning review trimmed two planner ideas: adding `writingsuggestions="false"` everywhere (no
requirement asks for it; the terminal sets it for an unrelated input-corruption hypothesis), and a new per-dialog test
sweep (an existing spec already walks these dialogs; see Tests).

## Outline

Line numbers drift; find the code by name. A survey of main at 95bb60a7 found 24 text-entry fields, listed below.

**Already complete** (`autocomplete="off"`, `autocorrect="off"`, `autocapitalize="none"`, `spellcheck="false"`): rename
(`rename.rs`), the sign-in token field (`auth.rs`), the model combobox (`launch_controls.rs`), restart-with's two
command fields (`restart_with.rs`), the host destination and add-host fields (`hosts.rs`), and the launcher's search,
repository, folder, name, agent command and resume command fields (`list/create_form.rs`). xterm's own hidden textarea
is handled by xterm and `terminal.js`; leave it.

**Partial:** the checkout folder field (`CheckoutRootField` in `crates/farhelm-ui/src/checkout_root.rs`, used in
Settings and in the launcher's managed-checkout setup) sets only `autocomplete` and `spellcheck`.

**Missing all four:**

- the template dialog (`crates/farhelm-ui/src/list/templates.rs`): the template name, and the model, command, resume
  command, session name and destination fields in its field editor;
- the "save as template" panel's name field (`crates/farhelm-ui/src/list/save_template.rs`);
- the quick switcher's search field (`crates/farhelm-ui/src/list/quick_switcher.rs`; its `aria_autocomplete: "list"` is
  the ARIA attribute and stays);
- the feedback dialog's message box and contact field (`crates/farhelm-ui/src/feedback.rs`).

Re-survey before starting (grep the UI crate for `input {` and `textarea {`, and check for fields added since this was
written), then add the missing attributes to every field. There is no shared helper today: each complete site repeats
the attributes inline, most with a short comment on why. Whether to introduce one (a constant set of attribute values,
or a small wrapper) or keep repeating them inline is your call; pick whichever keeps the rsx readable and the next field
easy to get right, and log the DECISION. Do not build a general input component for this.

The desktop webview (`crates/farhelm-ui/src/desktop.rs`) configures nothing about spell checking. Per-element attributes
are what both engines honor, so no webview-level setting is needed; if you find the desktop app still corrects text
after the attributes are in place, that is a scope reassessment, not a silent addition.

Tests: `e2e/tests/tooltip-coverage.spec.ts` ("sidebar surfaces give every control hover text") already walks almost
every surface with text fields: rename, host settings and add host, feedback, Settings (where the checkout folder field
always shows), the templates dialog and its field editor, both launcher tabs, and save as template. At each step it runs
a page-wide check (`expectCovered`) that cannot pass on an empty page and is proven to report a bad control. Add a
second check of the same kind at those steps (or a new spec reusing that walk, if mixing the two concerns reads badly;
log which), asserting all four attributes on every text-entry field in the open surface. Extend the walk to the template
editor's command, resume command, session name and destination fields, and open the quick switcher once.

Two traps:

- Seven of the fields this plan fixes have no `type` attribute (the quick switcher and five template fields), so a
  selector of `input[type=text], input[type=search]` skips exactly them. Select text-like inputs including
  `input:not([type])`, plus every `textarea`.
- Give each step a minimum field count, and prove the check fails on a field missing an attribute, as the existing
  coverage check does for its own condition.

Keep the email exception explicit in the check. Opening the launcher in managed-checkout mode is not needed: it only
shows the same checkout folder component that Settings already shows. The existing launcher-only test in
`e2e/tests/terminal.spec.ts` ("create form inputs opt out of autocomplete, autocorrect, autocapitalize, and spellcheck")
becomes redundant once the walk covers the launcher; remove it in the same PR unless it proves something the walk does
not, and log the DECISION.

The PR removes the TODO.md entry this plan covers. Its changelog fragment is `kind: fixed` or `changed`, your call.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`;
- `cargo check -p farhelm-ui --features desktop`, since the UI crate's desktop build compiles the same components;
- the new or changed Playwright spec on Chromium and WebKit through the recorder, after the builds the browser suite
  needs, plus `e2e/tests/templates.spec.ts` and the feedback dialog's spec if the change touched how they select fields;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
- `python3 releasing/check-changelog.py format`.

Do not run the full battery without a specific risk that calls for it.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-text-fields-no-spellcheck-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
work.

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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/text-fields-no-spellcheck/<nn>-<short-name>`, numbered in the order under Outline (for example
  `plan/text-fields-no-spellcheck/01-no-spellcheck`).
- One PR, or a short linear stack if the test change reads better on its own. Keep them bite-sized and avoid churn: code
  added in one PR and deleted in a later one means the stack should have been shaped differently. Within this run, a PR
  that needs correcting is restructured rather than corrected on top, and that applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the PR's part of the outline.

For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Ask the reviewer to confirm that no text field the UI renders was missed (it should grep for
itself) and that the browser test would fail if one were.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a webview-level setting, a general
  input component, or edits well outside the files the outline names beyond tests and bookkeeping (examples, not a
  blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered. In particular:

- whether the attributes are shared through a helper or repeated inline;
- where the browser sweep lives and which dialogs it covers;
- the Conventional Commit type and changelog kind;
- which browser specs ran;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: leaving spell checking on in some field, or a
webview-level setting that changes behavior beyond text fields.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
