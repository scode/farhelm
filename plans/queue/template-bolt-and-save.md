# Templates: a ⚡ that shows whether it is lit, and a Save that shows whether anything needs saving

Written against main at 255fb3a7 on 2026-10-30. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Two TODO.md entries (bucket "Near term"), planned together with the maintainer's agreement because both change the
Templates panel and its tests. Verbatim:

> **Make the template list's ⚡ show clearly whether it is lit.** In the Templates list, the quick-launch bolt looks
> almost the same whether or not that template is the marked one: it always shows as filled, and only its size wiggles
> slightly. The unmarked state needs to be clearly different (an outline, say), so the list shows at a glance which
> template ⚡ launches.

> **Show in the template editor whether there is anything to save.** The editor always shows "Fields not shown are left
> as is. Another template can supply choices this one leaves out." just above the footer, so after clicking Save the
> dialog looks unchanged and it feels like nothing happened. Grey out Save when the draft has no unsaved changes, so its
> state shows whether a save is needed and that one went through.

How it works today, from a read of main at 255fb3a7 and an independent planning review (line numbers approximate):

- The Templates list renders the bolt as a plain `button.templates-quick-mark` whose text is the character "⚡"
  (`crates/farhelm-ui/src/list/templates.rs` around 2393-2405); lit vs unlit is only `aria_pressed`. The CSS
  (`crates/farhelm-ui/assets/app.css` around 3487-3496) colors it `--fg-2` unlit and `--warn` lit, but U+26A1 renders as
  a color emoji, so the color never applies. The button is already a fixed 28px flex item; the "wiggle" is the emoji
  glyph's own metrics when the lit row grows a line for its chord (around 2417).
- The editor already tracks a draft against a saved baseline: `dirty = opened() && current != baseline()` (around 1928)
  drives "Unsaved changes" (around 2434), and the leave prompt uses the same comparison (around 1961, 2488). Save
  (around 2501) is disabled only while busy. Opening New gives a clean draft; Duplicate's baseline is the default so a
  copy is dirty; raw wrapper or container text that does not parse counts as dirty.
- Precedent for an unavailable Save: `crates/farhelm-ui/src/checkout_root.rs` (around 80-94) uses
  `aria_disabled: locked || !edited()` and returns early from the click handler; `checkout_post_clone.rs` does the same.
- SPEC.md's Templates section (around 816-895) says nothing about how a lit or unlit bolt looks, nor that Save is
  unavailable when clean; it says "existing names with whitespace remain usable, save unchanged".

### Acceptance criteria

- In the Templates list, the bolt is a drawn icon, not the emoji character:
  - unlit, an outline bolt in the muted foreground color;
  - lit, a filled amber bolt (the existing `--warn` color). It stays the same size and in the same place whether lit or
    not, and whether or not its row shows the chord. Hover, focus, the disabled look, `aria-pressed`, tooltips and click
    behavior are unchanged. Other places that show "⚡" (the quick switcher's label, prose hints, tooltips, keyboard
    shortcut lists) keep the character.
- In the template editor, Save is greyed out and does nothing while the draft has no unsaved changes, using
  `aria-disabled` and an early return as `checkout_root.rs` does, so focus stays on it after a save completes. It is
  available whenever "Unsaved changes" shows. The leave prompt's own Save is unaffected (it only appears for a dirty
  draft).
- Browser tests: one asserting the lit and unlit bolts render differently (for example by computed fill) while
  `aria-pressed` still reflects the mark; one asserting Save is unavailable on a clean draft, becomes available after an
  edit, and is unavailable again after saving. Existing tests that click Save on a clean draft are adjusted to the new
  rule.
- SPEC.md: the Templates section says how the lit and unlit bolt look, and that Save is unavailable while there are no
  unsaved changes; reword "existing names with whitespace remain usable, save unchanged" so it no longer implies saving
  an unedited template.
- A changelog fragment per root `AGENTS.md`, "Changelog fragments", and the last code PR removes both TODO.md entries.

## Decisions already made

Settled with the maintainer on 2026-10-30, on the planning page:

- **One plan, two PRs** for the two entries.
- **Bolt look A**, as mocked on the planning page: outline in the muted color when unlit, filled amber when lit.
- **No "Saved" note:** a greyed-out Save and "Unsaved changes" disappearing are enough feedback.
- **Only the Templates list's bolt changes** (from the planning review; the quick switcher's ⚡ is a label, not an
  on/off indicator).
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the two TODO.md entries quoted above and the decisions above.

**Binding repository constraints:** SPEC.md's Templates section (around 816-895); `docs/harness-marks.md`, which root
`AGENTS.md`, "Instructions by path", requires reading before editing `crates/farhelm-ui/src/icons.rs` or
`crates/farhelm-ui/src/icons/`; `docs/desktop-web-triage.md`; root `AGENTS.md`, "Tests and validation", "Conventional
Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`; `docs/test-sleep-check.md` and
`scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- One SVG bolt with explicit width and height, its outline or fill switched by CSS on `aria-pressed`
  (`fill: none; stroke: currentColor` unlit), placed with the existing icon components in `icons.rs` if that fits
  `docs/harness-marks.md`, otherwise inline in `templates.rs`.
- Known and out of scope: the initial-open path sets the baseline to the normalized draft (around 1810-1812) while
  opening from the list uses the raw stored fields, so an inferred-kind draft reads as clean only on the first path.
  Mention it in the report's follow-ups; do not fix it here.

## Outline

1. `plan/template-bolt-and-save/01-bolt-icon`: the drawn bolt, its CSS, the browser test, the SPEC.md sentence, the
   changelog fragment, and the removal of the bolt's TODO.md entry.
2. `plan/template-bolt-and-save/02-save-when-dirty`: Save unavailable on a clean draft, the browser tests, the SPEC.md
   sentences, the fragment, and the removal of the Save entry.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-ui`, `python3 releasing/check-changelog.py format`, `python -B scripts/check-test-sleeps.py` with the
interpreter `docs/test-sleep-check.md` prepares, and, through `python3 scripts/record-test-run.py`,
`e2e/tests/templates.spec.ts`, `e2e/tests/quick-launch.spec.ts` and the other specs that click `.templates-save` or
`.templates-quick-mark` (`popover-template-movement`, `github-checkouts`, `modal-trap`, `custom-permissions`), on both
engines.

## Fallbacks

Examples of departures that need the scope reassessment: changing the bolt elsewhere, a "Saved" note, changing the leave
prompt, or changing how the baseline is computed.

Decisions to log: where the icon lives, the exact colors and stroke, every test changed and why, and every review
finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-template-bolt-and-save-log.md` in the parent directory of the checkout you run in, derived as that section
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
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/template-bolt-and-save/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

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
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

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
passed its review gate, and the code PRs remove this plan's two TODO.md entries. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
