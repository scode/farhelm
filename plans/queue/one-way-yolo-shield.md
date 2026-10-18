# One-way observed YOLO shield

Written against main at 63439c2f on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Within one agent run, the sidebar shield's reading of the agent's status line only ever moves toward YOLO. Today a
definite no-checks reading turns the shield YOLO and a later recognized non-YOLO reading puts the launch shield back;
after this plan, once a run has shown a no-checks mode, its shield stays YOLO until the run ends (Restart, Replace,
resume) or the host's supervisor restarts. The spec and docs record why.

TODO.md entry this plan covers (Near term): "**One-way observed YOLO shield.** The sidebar shield now reads the agent's
status line (#2063): seeing a no-checks mode turns the shield YOLO, and a later recognized non-YOLO mode restores the
launch shield (SPEC.md, Session list, the paragraph on reading the permission mode from the status line). The intent was
that this reading only ever moves the shield toward YOLO, for safety. Discuss the details first, including Claude's plan
mode (which keeps the last reading) and Cursor (which shows no label when its mode is off), then decide whether to make
it one-way."

### Acceptance criteria

- `ActivitySample::observe_permission` in `crates/farhelm-supervisor/src/service/ticker.rs` latches: a definite YOLO
  reading sets `screen_yolo`, and nothing else in the same run clears it. A definite non-YOLO reading, an unreadable
  screen and an unrecognized mode all leave it as it is. It still reports a change (so connected helms get a fresh list)
  only when the value actually changes.
- The latch is decided in the supervisor, not in the UI, so every client shows the same shield.
- It applies to every harness whose status line Farhelm reads (Claude, Grok, Muse, Cursor; `status_line_yolo` in
  `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs`). Codex and the others stay unread.
- Unchanged: the reading lives only in memory; `ActivitySample::replacement` (Restart, Replace, resume) and
  `ActivitySample::reloaded` (supervisor restart) start it at false; a rename shares the run cell; it stays display only
  (no admission, inheritance, approval or host YOLO guard reads it); it never makes a shield safer than the launch.
- The existing unit test `permission_evidence_reaches_replies_and_resets_with_the_run` (in `ticker.rs`) is flipped: a
  bypass footer, then a manual-mode footer, keeps `screen_yolo` true and reports no change; a second bypass footer
  reports no change; replacement and reload still reset it. Its docstring is updated to the latch. The harness-specific
  footers already have their own fixture test (`captured_permission_footers_match_and_ignore_transcript_mode_words` in
  `crates/farhelm-supervisor/src/agent_kind/status_line_fixtures.rs`), and the latch does not look at the harness, so no
  per-harness latch cases are needed; one optional loop over the three `not-yolo-status.txt` fixtures is fine.
- The hover and screen-reader text in `agent_badge` (`crates/farhelm-ui/src/list/row.rs`) may add "during this run"
  after "YOLO observed on the agent's status line", keeping that prefix, so its unit test near line 2636 stays as it is.
  This is optional; log what was chosen.
- Doc comments that become wrong under the latch are updated: the `screen_yolo` field doc on `ActivitySample` ("Last
  definite permission-footer reading of this run"), the `status_line_yolo` doc ("callers retain their last answer until
  another footer says otherwise"), and the `screen_reader.rs` module doc (around lines 48-51, "an obscuring dialog
  retains the last mode").
- SPEC.md, Session list (the paragraph that begins "For Claude, Grok, Muse and Cursor, Farhelm also tries to read the
  agent's permission mode"): "The latest definite reading wins: a recognized non-YOLO mode restores the launch shield"
  is replaced by the one-way rule and its reason (see Requirement sources); the Cursor sentence, which today explains
  why Cursor alone sticks, is folded into the general rule. SPEC_impl.md's "Permission-footer evidence" bullet says the
  bool latches within a run, and its sentences "Cursor has no definite non-YOLO label, so absence cannot clear its
  reading. Unrecognized modes and covered footers retain the previous answer." become part of that one rule.
- The user docs say the same: `website/src/content/docs/docs/agents/index.mdx` (lines around 138-144, which say
  returning to a mode Farhelm recognizes as asking first restores the launch shield) and the Cursor note in
  `website/src/content/docs/docs/agents/cursor.md` (around lines 21-23), which no longer needs to be special.
  `claude.md`, `grok.md` and `muse.md` were checked during planning and say nothing about the shield going back; they
  need no edit.
- A changelog fragment per root `AGENTS.md` (Changelog fragments). #2063 shipped in v0.29.0, so this is a new fragment,
  not an edit of #2063's.
- The last PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-17):**

- Make the observed reading one-way, for all four read agents. The user's reason, to go into the spec: "we want to err
  on the side of safety. If we actually have positive evidence that a session qualifies for YOLO, it is only positive to
  do so; it is promoting safety. Whereas if we ever go the other way, we open up the problem that a YOLO session could
  turn non-YOLO and then we fail to turn it back into YOLO, thus violating a promise we're effectively making to users."
- Keep the reading in memory, as today: a supervisor restart clears it until the status line shows YOLO again. It is not
  persisted per run.
- Restart, Replace and resume still start the new run from its launch shield.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:** SPEC.md, Session list, the shield paragraph quoted above, including that the reading
is advisory, best effort and display only, and never makes a shield safer than its launch choice. SPEC_impl.md,
"Permission-footer evidence". Root `AGENTS.md`: Code rules (per-harness behavior is answered per harness in the named
places, never by comparing harnesses at the call site; the latch is harness-independent, so it needs no harness branch),
Tests and validation, Changelog fragments.

**Planner proposals (checked by a fresh-context planning review):**

- Change only the store, not the parser: `status_line_yolo` keeps its three answers (YOLO, definitely not, can't tell),
  and `observe_permission` ignores "definitely not". Keeping the parser intact keeps its fixtures meaningful and the
  diff small. A one-line latch (on a YOLO reading with the bool still false, set it and report a change; otherwise
  report no change) is the whole behavioral change.
- Claude's plan-mode special case (plan mode reads as "can't tell" rather than non-YOLO) becomes irrelevant to the
  shield, because a non-YOLO reading no longer clears it. Leave the parser's plan-mode rule alone; its doc comment
  ("keeps the last reading") stays true, and a short note that it no longer affects the shield is optional.
- Checked during planning: the only write to `ActivitySample::screen_yolo` is inside `observe_permission` (`ticker.rs`,
  around line 472); every other `screen_yolo: false` in the supervisor builds a `SessionInfo`, and `status.rs` only
  copies the sample out. Helm merges take the incoming value as given (SPEC_impl.md, "Permission-footer evidence"), so a
  latched true is never hidden by a cached false.
- The PR type is `fix` (the shipped behavior did not match the stated intent), with a fragment whose `kind` is
  `changed`: users will notice that leaving bypass mode no longer turns the shield back. The draft entry says the shield
  now stays YOLO for the rest of the agent run once a no-checks mode was seen, and resets on restart.

## Outline

One `fix` PR: the latch in `observe_permission`, the doc comments in `ticker.rs` and `screen_reader.rs`, the flipped
unit test in `ticker.rs`, the optional hover wording in `row.rs`, the SPEC.md and SPEC_impl.md edits, `index.mdx` and
`cursor.md`, a changelog fragment (`kind: changed`), and the removal of the TODO entry.

### Validation

Choose per `.agents/validation.md`; at least: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
the `farhelm-supervisor` `ticker` and `screen_reader` unit tests and the `farhelm-ui` `list::row` unit tests through the
recorder (`--tmux` as each needs), `python3 releasing/check-changelog.py format`, `dprint check` on changed Markdown,
and the website build (`cd website && bun install --frozen-lockfile && bun run build`) because docs pages change. No
browser tests: no browser test covers the observed shield today, and the change is in the supervisor's stored bool and a
hover string, both unit-tested. Not the full suite.

## Fallbacks

No plan-specific fallbacks are agreed beyond the general unattended rules; the planning checks above settled where the
bool is written and how helms merge it. If the code turns out to differ from those checks, make the latch hold on every
path within a run and log the DECISION.

Examples of departures that need the scope reassessment: persisting the reading across supervisor restarts, a manual
"clear observed YOLO" action, reading Codex's status line, changing the parser's readings.

Decisions to log: the latch's exact shape, the final hover wording.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-one-way-yolo-shield-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/one-way-yolo-shield/<nn>-<short-name>` (for example `plan/one-way-yolo-shield/01-one-way-reading`).
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
