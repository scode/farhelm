# Characters new terminals drop: state one, restore the other

Written against main at d81a3980 on 2026-11-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Execute two triage outcomes recorded on 2026-11-04, per `review_feedback_queue/.agents/execute-outcomes.md`, "Execute
triage outcomes" (one PR per outcome). Their `TRIAGE_OUTCOMES.md` entries are the binding statement of what to do; read
each in full from `main@origin`, together with its feedback file under `review_feedback_queue/`:

1. `selector-policy-drops-ordinary-formatting-input.md` (`fix spec`)
2. `emoji-continuation-drops-standalone-kirat-characters.md` (`fix code`)

The maintainer picked "Keep visible letters; accept dropping invisible formatting" on the 2026-11-04 page. Both drops
came with v0.35.0's emoji-width work (#2971) in the vendored terminal emulator.

### What each PR does

1. **Invisible formatting characters.** New terminals drop zero-width space, directional marks, embeddings and isolates,
   line and paragraph separators, word joiner, BOM and two Hangul fillers before storing them
   (`vendor/alacritty_terminal/src/term/mod.rs`, `emoji_ignored_format` around 1443-1451, `Term::input` around
   1503-1516). State it: SPEC_impl.md (the emulator selector rules, around 2645-2654) and the user docs page on new
   terminals (under `website/src/content/docs/docs/`, `new-terminals`) say these are dropped, so copied text and history
   lack them. No code change. Read `website/AGENTS.md` before editing the docs page. `docs:` type.
2. **Kirat Rai vowel signs.** `Term::input` (around 1517-1524) sends every V/T character to `input_emoji_jamo` (around
   1248-1263), which keeps it only after Hangul L/LV/V/LVT/T or Prepend and drops it otherwise, and at column 0
   (`emoji_attachment_point`, around 1134-1137); `emoji_jamo_property` (around 1469-1490) classes Kirat Rai vowel signs
   as V. A V/T that cannot attach prints as an ordinary character at its own width again, as before #2971, while a
   following selector still finds the emoji base. Update the three tests that pin the drop
   (`emoji_jamo_drops_illegal_and_baseless_suffixes`, `emoji_selectors_follow_rejected_jamo`,
   `emoji_jamo_legacy_replay_keeps_scalar_behavior`) and the vendored patch file together, per `vendor/README.md`.
   `fix:` with a changelog fragment.

### Acceptance criteria

- Two PRs, each meeting its ledger entry's completion criteria (item 2 with a 100-line gate: over it, block and bring it
  back), each recording its execution in the ledger, and each removing its feedback file and index line.
- Vendored changes go with their patch file (`vendor/README.md`; root `AGENTS.md`, "Frozen material").

## Decisions already made

- **The outcomes**, chosen by the maintainer on 2026-11-04 (quoted above).
- **Grouping:** the maintainer approved these two as one plan on the 2026-11-04 planning page.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the two `TRIAGE_OUTCOMES.md` entries named above.

**Binding repository constraints:** `vendor/README.md`; `website/AGENTS.md`; SPEC_impl.md's emulator selector rules;
root `AGENTS.md`, "Tests and validation", "Changelog fragments", "Conventional Commits"; `.agents/test-authoring.md`;
`review_feedback_queue/.agents/execute-outcomes.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): item 2 restores the pre-#2971
behavior for an unattachable V/T rather than inventing a new rendering.

## Outline

Two PRs, bottom to top:

1. `plan/terminal-text-drops/01-state-formatting-drop`
2. `plan/terminal-text-drops/02-print-unattached-vowel-signs`

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect, for item 2, the three vendored-crate test
commands, the "terminal emulator" selection and "emulator parity with tmux" through
`python3 scripts/record-test-run.py`; for item 1, `dprint check`, plus the website build if the edit adds or changes a
link or heading.

## Fallbacks

Examples of departures that need the scope reassessment: keeping the invisible formatting characters (the maintainer
accepted dropping them), changing selector widths, or a snapshot format change.

Decisions to log: how item 2 falls through to ordinary printing, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-terminal-text-drops-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/terminal-text-drops/<nn>-<short-name>`,
  as the outline names them.
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

The plan is complete when a linear stack of draft PRs exists, one per triage outcome in the order the outline gives,
each meeting that outcome's completion criteria in `TRIAGE_OUTCOMES.md` (as refined by Decisions already made above),
each having passed its review gate, and each recording its execution in the outcome's Execution field (change id,
bookmark and PR URL) and removing its feedback file and `review_feedback_queue/INDEX.md` line, per
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes". Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.

## Decisions

### 2026-11-05: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-11-05 (claim 7e32f1).
>
> ### Restoring vowel signs changes which character a selector can affect
>
> New terminals currently delete standalone Kirat Rai vowel signs from the screen and copied text, including in an
> ordinary word with no emoji. Triage chose "Keep visible letters; accept dropping invisible formatting". The
> independent documentation outcome is finished in [draft PR #3029](https://github.com/scode/farhelm/pull/3029/changes):
> the specification and the Move sessions to new terminals guide now state the accepted loss of invisible formatting
> from copies, scrollback and saved terminal contents. No terminal code has changed; the vowel-sign bug remains.
>
> The remaining outcome requires an unattached vowel or trailing sign to print at its ordinary width while a following
> emoji selector still finds the earlier emoji. These requirements conflict with the specified rule that a selector uses
> the immediately preceding stored character. For example, printing a heart, then the one-column Kirat Rai sign U+16D67,
> then the emoji selector U+FE0F would retain the sign between the heart and selector. The selector would therefore be
> ignored. Reaching past the sign to widen the heart would change selector eligibility and need layout rules to keep the
> new wide heart from overwriting the sign. A zero-width Hangul sign restored as an ordinary combining mark also becomes
> the predecessor and blocks a following selector. This is established by source inspection and an independent read-only
> investigation; no new runtime experiment was run.
>
> Which behavior should the repair implement?
>
> - **Restore ordinary printing for every unattached sign.** Keep the existing immediate-predecessor selector rule: a
>   selector after a retained sign is ignored. This gives the requested pre-change scalar behavior, including characters
>   that ordinary printing stores as combining marks, but drops the requirement to reach the earlier emoji. Recommended:
>   it preserves the text without introducing a new selector or layout policy.
> - **Restore only printable, nonzero-width signs.** Continue dropping rejected zero-width signs, so selectors can still
>   reach an emoji across those; a retained Kirat Rai sign blocks the selector. This fixes the visible-text bug and
>   keeps the current zero-width selector cases, but narrows both the ordinary-printing requirement and the selector
>   guarantee.
> - **Require selectors to reach past retained signs.** Authorize a separate selector/layout design and reassess its
>   scope and snapshot constraints before implementing it. This needs rules for preserving the intervening text when an
>   emoji grows, including at the end of a row, and is wider than the agreed small repair.
>
> The existing roughly 100-line gate still applies to a clarified repair, including code, tests, documentation and the
> vendored patch; no exemption for mirrored patch lines or documentation references has been assumed. The paired parser
> test and a snapshot test also pin today's deletion, beyond the three tests named in the plan. They must change with
> the clarified behavior. The invisible-formatting documentation passed formatting and whitespace checks; the
> instruction checker reported zero errors and three existing unrelated warnings. No runtime tests or website build were
> needed for that prose-only change, which adds no links or headings.

The maintainer's answer:

The maintainer chose the first option on 2026-11-05, in conversation with the landing monitor: "(a) please".

Restore ordinary printing for every unattached vowel or trailing sign, and keep the existing rule that an emoji selector
applies only to the character immediately before it. A selector that follows a retained sign is ignored, so in the
heart, Kirat Rai sign, selector example the heart stays narrow and the sign stays visible. Characters that ordinary
printing stores as combining marks, such as the zero-width Hangul signs, are restored that way too, and a selector after
them is ignored as well. The requirement that a selector still reach the earlier emoji across a retained sign is
dropped; do not add a new selector or layout rule for it.

For context, the monitor read Ghostty's current source before the maintainer answered. Ghostty counts these signs as
zero width and, in its default grapheme mode, discards a zero-width character that starts a new cluster, so it loses the
same vowel signs this plan restores. Matching Ghostty here was considered and not chosen: visible text is kept.

Update the tests that pin today's deletion (the three the plan names, the paired parser test and the snapshot test) to
the restored behavior, and state the selector rule where the specification describes selectors. The roughly 100-line
gate still applies as the Blocked section describes, with no exemption for the vendored patch or documentation.

### 2026-11-05: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-11-05 (claim 85f8c7).
>
> ### Restoring vowel signs exceeds the retained change-size limit
>
> New terminals currently delete standalone Kirat Rai vowel signs from the screen and copied text, including inside an
> ordinary word with no emoji. Triage chose to preserve visible letters while accepting loss of invisible formatting.
> The accepted formatting-loss documentation is finished in
> [draft PR #3029](https://github.com/scode/farhelm/pull/3029/changes), which remains open and draft. It changes no
> terminal behavior; the vowel-sign bug remains.
>
> The previous answer settled the behavior: every unattached vowel or trailing sign uses ordinary character printing,
> including zero-width signs stored as combining marks. A selector following a retained sign is ignored. The same answer
> explicitly kept the roughly 100-line limit, counting the mirrored vendor patch, tests and documentation.
>
> A concrete private proposal for that behavior measures 320 physical added-plus-deleted lines, before two lines for the
> patch's regenerated hash and any formatting or review corrections:
>
> | Part                                                            | Changed lines |
> | --------------------------------------------------------------- | ------------: |
> | Terminal implementation and its three existing regression tests |           105 |
> | Required mirrored vendor patch, including moved hunk headers    |           115 |
> | Vendor documentation, mostly moved patch references             |            58 |
> | Paired parser regression expectations                           |             6 |
> | Saved-state regression test                                     |            33 |
> | Selector specification                                          |             3 |
>
> The repair uses one shared, read-only attachment check. A sign attaches under the existing legal-pair rules or falls
> through to existing ordinary printing. The repeat command uses the same check before printing: a standalone printable
> Kirat sign becomes the repeated character, while an attached sign keeps the prior character. Existing legal Hangul
> attachments stay intact. There is no new remembered state, selector layout rule or snapshot format.
>
> The code and its three tests alone require 210 lines when mirrored, before the other required files. Smaller wording
> or test edits may shave that down, but no credible complete proposal near 100 lines was found. A helper that changes
> the grid cannot also answer the repeat command's read-only pre-print query; duplicating the rules would create two
> definitions of attachment. Omitting required tests, leaving patch references stale or exempting mirrored lines would
> weaken or bypass the recorded constraints.
>
> Authorize a ceiling of **400 physical changed lines for this repair**, with all existing counting rules and behavior
> unchanged, or keep the 100-line ceiling and defer the repair. I recommend 400: the measured proposal is about 322
> lines with its patch hash, and the remaining room covers normal formatting and review corrections. Keeping 100 leaves
> the known text-loss bug unresolved; it does not supply an alternative implementation.
>
> The proposal is sizing evidence, not a tested implementation. No repair was applied to the checkout, and no new
> runtime tests ran. Implementation still requires the plan's focused vendor/emulator checks and single fresh
> gpt-6.1-sol/high review. The unchanged documentation PR passed formatting and whitespace checks; the instruction
> checker reported zero errors and three existing unrelated warnings. Its previous review remains applicable after an
> empty-interdiff rebase. No website build was needed because no links or headings changed.

The maintainer's answer:

The maintainer allowed the larger repair on 2026-11-05, in conversation with the landing monitor: "its fine allow the
lines".

Implement the measured proposal with a ceiling of 400 physical changed lines for this repair, counted under the existing
rules (code, tests, the mirrored vendor patch and its hash, vendor documentation and the specification). The behavior
stays as the previous answer settled it: every unattached vowel or trailing sign uses ordinary printing, a selector
after a retained sign is ignored, and the repeat command uses the same shared attachment check. Add no new remembered
state, selector layout rule or snapshot format. The roughly 100-line limit the previous answer restated came from the
monitor carrying the plan's gate forward, not from the maintainer, and this answer replaces it for this repair. Run the
plan's focused vendor and emulator checks and its required independent review as before.
