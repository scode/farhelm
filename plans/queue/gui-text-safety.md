# Execute seven triage outcomes: safe pastes, escaped template text, and clean template commands

Written against main at ee306896 on 2026-10-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PR amends them as the ledger entries decide.

This plan has no dependency on another plan.

## The goal

Carry out seven triage outcomes recorded in root `TRIAGE_OUTCOMES.md` in one draft PR. Their headings, which are also
the feedback file names under `review_feedback_queue/`:

- `embedded-paste-terminators-allow-command-execution-definite.md` (`fix code`)
- `template-summaries-interpolate-model-text-into-approval-summary-model-interpolation.md` (`fix spec+code`)
- `template-summaries-interpolate-model-text-into-approval-templates-list-summary-rendering.md` (`fix spec+code`)
- `template-summaries-interpolate-model-text-into-approval-quick-switcher-summary-rendering.md` (`fix spec+code`)
- `ascii-spaces-remain-invisible-host-identity-approval-labels.md` (`fix spec+code`)
- `template-command-editors-hide-meaningful-characters-silently-saved-command-editor.md` (`fix spec+code`)
- `template-command-editors-hide-meaningful-characters-silently-saved-resume-command-editor.md` (`fix spec+code`)

All seven are about text the user cannot fully see or trust:

- Pasting text that contains the hidden end-of-paste marker (`ESC[201~`) into a terminal ends the paste early, so the
  rest of the clipboard reaches the program as typed input, for example running in a shell. The user chose to strip the
  marker in Farhelm's own paste path.
- A launch template's one-line summary shows its model name unescaped (in the summary, the Templates list and the quick
  switcher), and the adopt prompt shows installation identities with spaces unescaped, so two different identities can
  look alike. The user decided that all host-originated text the GUI shows is escaped.
- Template launch and resume commands are edited in single-line boxes, so a stored newline is lost silently on edit and
  invisible characters cannot be seen. The user decided that template commands may not contain control or invisible
  formatting characters, and saving one is refused with a clear error, from the GUI, the CLI or an agent.

Each ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do for
that outcome. Read the seven entries and feedback files before starting. Where this file and a ledger entry seem to
disagree, the ledger entry wins and the disagreement is a DECISION to log (or a gate trip, per Complexity gate).

Acceptance criteria:

- One draft PR carries out every outcome that did not trip its complexity gate, each meeting its ledger entry's
  completion criteria. The spec sentences those entries name already landed (see Requirement sources).
- Tests cover a paste containing the end marker (a `node --test` case), the escaped model name in a template summary,
  visible spaces in an identity label, and refusal of a template command with a newline or invisible character on the
  shared save path.
- The PR removes each carried-out outcome's feedback file and index line, updates every outcome's `TRIAGE_OUTCOMES.md`
  Execution field, adds a changelog fragment, and passed the review gate.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decisions (2026-10-10):** rather than triage the remaining highest-priority review findings one by
one, the user answered a series of product and spec questions, each settling a class of findings. Every outcome this
plan carries out is recorded in root `TRIAGE_OUTCOMES.md` under its feedback file's name, with the user's own words in
its Decision field. Those entries, not this file, are the authoritative statement of what was decided. Two rules the
user stated that apply across outcomes:

- "unsupported but these kinds fo things need to be rejected, not just \"undefined behavior\"" (for configurations
  Farhelm does not support; one outcome, the `ProxyCommand` aliases, is an explicit exception the user accepted).
- On permissions: good permission practice by default, and minor steps such as restricting a file right after creating
  it are fine, but no significant complexity on defence in depth against a user's broad umask.

**The user's plan-time decisions (2026-10-10):** these outcomes are grouped into three plans with one PR each,
overriding root `AGENTS.md`'s one-PR-per-outcome rule for them only, under the same rules as the first batch of plans
from that day: each outcome has a complexity gate, and an outcome that trips it is dropped from the PR while the rest
ships; review gate gpt-6.1-sol at high effort; no-workhorse mode, which `plans/AGENTS.md` requires.

**Spec text already landed:** the spec sentences that `fix spec+code` outcomes name were written into SPEC.md on
2026-10-10, ahead of the code, so that later triage sessions see the decisions (search SPEC.md for "Confirmed
2026-10-10"). This plan's PR makes the code match them and does not need to write them again. If the implementation
shows a sentence is wrong or unworkable, that is a gate trip for its outcome, not a spec edit to make on your own.

**Planner proposals** are the mechanisms and placements in the outline below, checked against main by a fresh-context
planning review.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes, as
overridden above; Releases and the changelog; Agent scratch space; the rule against tests that modify the test process's
own environment variables; SPEC.md and SPEC_impl.md are authoritative and changed only as decided), the user's global
rule to write documentation and comment prose with the `scode-voice` skill, `plans/AGENTS.md` (Executing one plan),
`review_feedback_queue/AGENTS.md` (Lifecycle), and `.agents/test-authoring.md` for test changes.

## Outline

Line numbers drift; find the code by name. Verified on main at ee306896.

- Paste: `crates/farhelm-ui/assets/terminal.js` already intercepts pastes for files and inserts text through
  `term.paste()` (see `insert`); plain-text pastes go to the bundled library's own handler, which does not remove an
  embedded end marker. Intercept plain-text pastes too, remove `ESC[201~` (whether to remove ESC more broadly is a
  DECISION; the user's ask is the marker), and insert through the same `term.paste()` path so bracketed paste and the
  pane's other modes still apply. Put the sanitising decision in a pure function in `term-bytes.js`, which is already
  readiness-gated, loaded wherever `terminal.js` runs, and covered by `crates/farhelm-ui/js-tests`; do not add a new
  asset module (that needs asset registration, a readiness gate and desktop asset parity). Apply it inside `insert()` so
  dropped text is covered too, and rewrite the comments that promise plain-text pastes keep the library's own handler.
  The bundled library stays unpatched.
- Template summary: `crates/farhelm-ui/src/list/templates.rs`, `template_summary`, uses `model.clone()` while the
  template name beside it is escaped. Pass the model through `crate::peer::display_peer`. The Templates list and the
  quick switcher (`crates/farhelm-ui/src/list/quick_switcher.rs`) render that same summary, so the one change covers all
  three outcomes; confirm that. Do not restructure the quick switcher's direction isolation.
- Identity spaces: `crates/farhelm-ui/src/peer.rs`, `display_identity`, keeps ASCII spaces as they are, contrary to its
  own doc. Make spaces visible in the same escaped style the function already uses for other characters (other ASCII
  whitespace is already escaped as control characters). Installation identities do not normally contain spaces, so
  ordinary labels do not change; the existing test expecting `display_identity("   ")` to read as whitespace only will
  change with it.
- Template commands: `farhelm_proto::launcher::check_template_shape` is already the shared check, called by the helm's
  `store_template` (used by the REST route and agent writes), by the agent path before its approval card, and by both
  GUI save paths. Extend it to refuse, with a clear message naming the field, a launch or resume command containing a
  control character or an invisible formatting character; `farhelm_proto::text::is_presentation_unsafe` may already
  classify that set, so use it if it fits. A template stored before this rule keeps launching, but editing any of its
  fields is refused until the command is fixed; say so in the PR description and the changelog.
- Spec: the two sentences these outcomes need already landed in SPEC.md (host text shown outside a terminal is escaped,
  identity labels show spaces; template commands refuse control and invisible characters). Check the code matches them.
- Changelog: `fix:` with `kind: fixed`, for someone running Farhelm: pasting text with a hidden end-of-paste marker can
  no longer run part of it as typed input; template summaries and the adopt prompt show hidden characters; templates
  refuse commands containing control or invisible characters.

The complexity gate for these is a small change each plus tests. Patching the bundled library, a new paste pipeline
beyond one interception, or enforcing the template rule separately in several places instead of one shared save path is
a gate trip for that outcome.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`, clippy with `--all-targets` on the touched
crates, focused nextest selections for the touched `farhelm-proto`, `farhelm-ui` and `farhelm-helm` tests through
`scripts/record-test-run.py` (`--tmux none`), and `cd crates/farhelm-ui/js-tests && node --test`. Run
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` and apply `.agents/test-authoring.md`.
`dprint check` on the changed Markdown. Browser validation per root `AGENTS.md`: if the paste interception changes what
reaches the terminal in a way unit tests cannot show, run a focused paste spec on Chromium and WebKit if one exists;
otherwise record why the unit tests suffice.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-gui-text-safety-log.md`
in the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before
you start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution fields on the PR) rather than starting over. If it does not exist, this is a
fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/AGENTS.md` (Executing one plan, step 7) before any work.

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
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/gui-text-safety/01-gui-text-safety`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); `kind: none` with a one-line reason is right when nothing changes for
  someone running Farhelm. Validate with `python3 releasing/check-changelog.py format`.
- For every outcome the PR carries out, it removes the feedback file under `review_feedback_queue/` and its line in
  `review_feedback_queue/INDEX.md`, and updates that outcome's `TRIAGE_OUTCOMES.md` Execution field to `complete` with
  the jj change ID, bookmark and PR URL. Record the change ID and bookmark before creating the PR, then add the URL to
  the same change and push again; no separate bookkeeping PR. An outcome dropped by its complexity gate is handled as
  Complexity gate says instead.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Complexity gate

Every outcome in this plan carries the user's complexity gate, stated in its ledger entry and in the outline above: the
fix should stay about as small as assessed at triage. If one turns out to need significantly more (a new mechanism,
cross-component plumbing, a change well outside the files the outline names, or a product or design decision), do not
grow it and do not block the whole plan for it. Leave that outcome out of the PR, keep its feedback file and index line,
and in the same PR set its `TRIAGE_OUTCOMES.md` Execution field to `pending`, waiting on the user, followed by what you
found and the realistic options with a recommendation (see `installer-startup-prune.md` in the ledger for the shape).
Name it among the open questions in the plan's report. The rest of the PR ships. Block per Unattended fallback only when
every outcome in this plan was dropped, since then there is no PR to deliver. Code that turned out to be already fixed
on main is not a gate trip: record the outcome as `discard` with "already fixed" and the fixing commit, per root
`AGENTS.md`, and remove its queue item.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria: each outcome's `TRIAGE_OUTCOMES.md` entry, the outline above, and
the user's decisions. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim, as root `AGENTS.md` requires. Also ask the reviewer to check that ordinary pastes, file pastes and
bracketed-paste behavior are unchanged apart from the removed marker, and that the template refusal holds on every save
path. Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here
or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new helper module, a
change to a shared formatter or protocol, edits outside the files the outline names beyond tests and bookkeeping), and
whenever the same component has needed repeated corrective review rounds, run a fresh-context review through
galaxy-brain with this charter, supplying the ledger entries, the user's decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A departure that
the review does not find necessary and small is a complexity-gate trip, handled as above.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular whether the paste fix removes only the marker or ESC more broadly, where the
template refusal lives and whether the GUI editor also refuses, the Conventional Commit type and changelog kind, every
outcome dropped by its complexity gate and why, and every review finding you decided not to follow. The user will ask
for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. For a single outcome, the agreed fallback is the complexity gate above. If current code or specs have
moved so that a recorded decision no longer applies, treat it like a gate trip for that outcome, per root `AGENTS.md`
(Execute triage outcomes); never re-triage an item yourself. Block per `plans/AGENTS.md` (Executing one plan, step 10)
only when no outcome is left to ship or the shared PR itself cannot proceed without the user.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome of this plan that did not trip its
complexity gate, meets the acceptance criteria above, and passed the review gate, with every outcome's
`TRIAGE_OUTCOMES.md` Execution field updated and the queue items of the carried-out outcomes removed. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
