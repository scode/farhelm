# Keyboard reach for the remaining scroll regions

Written against main at 78394ca6 on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

SPEC.md requires every scrollable region in the GUI to be reachable and scrollable with the keyboard alone, WebKit
included, with a tab stop of its own when its content holds nothing focusable. The keyboard-scroll-regions plan (#2190,
#2191) fixed the host setup plans and the trash list. An audit for this plan found the remaining regions that fall
short. Fix them and add a guard so new scroll regions are caught.

TODO.md entry this plan covers (Near term): "**Audit scroll regions for keyboard reach.** SPEC.md requires every
scrollable region in the GUI to be reachable and scrollable with the keyboard alone, WebKit included, with a tab stop of
its own when its content holds nothing focusable. The keyboard-scroll-regions plan fixed the host plans and the trash
list but audited nothing else. Already known to break the rule: the SSH connection help area while loading, showing an
error or showing no commands, the remove-host dialog's body, and the feedback dialog's "sent" message. Audit every
scroll region and fix the ones that fall short."

### Acceptance criteria

- These regions comply, each with a tab stop, `role="region"` and a specific `aria-label` only where its content can
  hold nothing focusable, and the existing inset focus ring:
  - the SSH connection help body (`crates/farhelm-ui/src/ssh_help.rs`), in all its states (loading, error, no hosts, no
    commands);
  - the remove-host dialog body (`crates/farhelm-ui/src/hosts/settings_dialog.rs`);
  - the feedback dialog's body in its "sent" state only (`crates/farhelm-ui/src/feedback.rs`), so the editing state
    keeps its Tab order;
  - the host settings dialog body while a change is saving and every control is disabled (`hosts/settings_dialog.rs`);
  - the approval card's initial-prompt box (`crates/farhelm-ui/src/approvals.rs`, rendered through `PeerBlock` in
    `crates/farhelm-ui/src/peer.rs`), which SPEC.md requires to stay its own scrollable block.
- The long ssh commands in the SSH connection help wrap instead of scrolling sideways (`.ssh-help-command pre`,
  `crates/farhelm-ui/assets/app.css`); the copy button still copies the exact command.
- A guard: a browser check in `e2e/tests/modal-layout.spec.ts` that opens the dialogs above in a small window (the SSH
  help in a text-only state, remove-host, feedback after sending, host settings while a save is held) alongside the ones
  it already opens, and fails on any overflowing auto/scroll element (either axis) that is neither a tab stop with a
  region role and name nor holds an enabled tabbable descendant. Menus and suggestion lists are exempt by their roles
  (`menu`, `listbox`), with no new marker in the page. The feedback dialog closes itself after 1.5 s, so its test holds
  that timer (`page.clock`).
- A Tab-and-PageDown test per fixed region with the existing helper `e2e/tests/helpers/keyboard-scroll.ts`
  (`expectKeyboardScroll`), on Chromium and WebKit.
- SPEC.md, next to the rule, names the exemptions: pop-up suggestion lists and menus move with their own arrow keys; the
  pointer-only file hover cannot be scrolled by anyone; the terminal scrolls with Shift+PageUp/PageDown. It also says,
  in one sentence, that on macOS keyboard-only users reach buttons with Keyboard navigation or Option-Tab, since by
  default macOS moves Tab only between text fields and pop-ups (`docs/manual-mac-checklist.md`), so button-only regions
  need no extra stop.

## Requirement sources

**The user's decisions (2026-10-17):**

- Fix all five regions the audit found, plus the host settings dialog while saving.
- A region gets its own tab stop only where its content can lack anything focusable.
- "The executor checks what the Mac app does and gives such regions their own stop if needed": settled during planning
  from the repository's own Mac checklist, which records the macOS default; no extra stops are needed, and SPEC.md says
  why.
- The guard and per-region tests above; exemptions written in SPEC.md next to the rule.
- Wrap long ssh commands instead of scrolling sideways.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:** SPEC.md's keyboard-scroll rule (around its line 1241) and the approval card's prompt
block. Root `AGENTS.md` (Tests and validation, Changelog fragments).

**Planner proposals (checked by a fresh-context planning review):**

- No shared component: write the three attributes inline at each site, as #2190 did; two are conditional. The inset ring
  at `app.css` around line 3309 (`.host-dialog-framed > .host-settings-body[tabindex="0"]:focus-visible`) already covers
  the framed dialog bodies; host settings uses a different dialog class and needs one more selector. Do not move the
  earlier plan's regions.
- `PeerBlock` takes only a class and text: either give it optional label props, or move the scrolling onto the wrapping
  element; keep the prompt its own scrollable block.
- `expectKeyboardScroll` checks vertical scrolling only, which is all the regions need once the commands wrap.

## Outline

1. `fix`: the attributes and focus ring for the five regions, the command wrapping, and their per-region tests;
   changelog fragment (`kind: fixed`).
2. `fix`: the guard in `modal-layout.spec.ts`, the SPEC.md exemptions and macOS sentence; edits the first PR's fragment
   if it needs to mention anything. Removes the TODO entry.

### Validation

Choose per `.agents/validation.md`; at least: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo check -p farhelm-ui --features desktop`, `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`,
`python3 releasing/check-changelog.py format`, and on Chromium and WebKit through the recorder: `modal-layout.spec.ts`,
`ssh-help.spec.ts`, `feedback.spec.ts`, `approval-layout.spec.ts`, `provisioning.spec.ts`, `trash-dialog.spec.ts`, and
the specs you changed. Not the full suite.

## Fallbacks

- If the audit turns up a further failing region while building the guard, fix it the same way in this plan and log it.

Examples of departures that need the scope reassessment: a shared scroll-region component with migration of existing
regions, a static stylesheet check, tab stops on every dialog body.

Decisions to log: the `PeerBlock` approach, the guard's exemption rule.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-keyboard-scroll-audit-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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
  `plan/keyboard-scroll-audit/<nn>-<short-name>` (for example `plan/keyboard-scroll-audit/01-scroll-regions`).
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
