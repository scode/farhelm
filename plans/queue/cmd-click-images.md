# Cmd-click opens common image types by default

Written against main at 1ba949ff on 2026-11-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term", subsection "Trivial"), verbatim:

> **Add common image formats to the default Cmd-click open list.** The desktop app's **File types to open with
> Cmd-click** field defaults to `md, txt, json`; add jpeg, png and a few more common safe image formats to that default
> (gif, webp, bmp and avif are the obvious candidates; svg stays out, being HTML-like and already warned about). The
> default is `DEFAULT` in `crates/farhelm-proto/src/open_extensions.rs`, and its warning classification needs to treat
> the new types as safe, or the settings dialog warns about its own default.

### What planning found (read from the code at 1ba949ff, not run)

`crates/farhelm-proto/src/open_extensions.rs` holds `DEFAULT` (`"md, txt, json"`, used by `from_stored(None)`, so it
applies to every helm whose preference was never saved, upgraded ones included) and `warning_kind`, which treats `md`,
`txt`, `json` and `log` as `Safe` and `html`, `htm`, `xhtml`, `svg` as `Html`. The default's text also appears in:

- SPEC.md, three places (search "md, txt, json" and "defaults to .md, .txt and .json"): the Settings paragraph (search
  "File types to open with Cmd-click"), the downloads paragraph, and the dated entry under "Maintainer-confirmed
  decisions", "Local authority and trust between hosts" (around 4868-4873). Each also names the warning-free types
  (`md`, `txt`, `json` and `log`), which criterion 2 changes too. The confirmed-decision entry is amended at the
  maintainer's request (this TODO entry); say so in the amendment, dated, rather than rewriting it silently;
- `website/src/content/docs/docs/using/session-list.mdx` (search "defaults to `md, txt, json`");
- tests: `open_extensions.rs`'s own. `crates/farhelm-ui/src/settings.rs` (around 1036) asserts `md, txt, json, log` is
  warning-free and builds only with the `desktop` feature on a native target, so a plain `cargo test -p farhelm-ui` runs
  none of it; `crates/farhelm-helm/src/downloads.rs` (around 982) checks that a refusal names `md, txt, json`, which
  still holds with the new default as a prefix.

### Acceptance criteria

1. The default is `md, txt, json, png, jpg, jpeg, gif, webp, bmp, avif`, in that order.
2. `png`, `jpg`, `jpeg`, `gif`, `webp`, `bmp` and `avif` are classified as safe, so the settings dialog shows no warning
   for the default; `svg` keeps its HTML-like warning.
3. A saved list, including an empty one, is unchanged; only a never-saved preference picks up the new default.
4. SPEC.md, the docs page and the tests above say the same.
5. A changelog fragment says what changed for someone running the desktop app.
6. The PR removes this plan's TODO.md entry.

## Decisions already made

Settled with the maintainer on the 2026-11-10 planning page:

- **Types:** png, jpg, jpeg, gif, webp, bmp, avif. Rejected: adding heic, tif and tiff; and only png, jpg, jpeg.
- **Reviewer:** a single fresh-context agent on Opus 5.5 at high effort (see Review gate). If it cannot be reached when
  a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled on hold**, at the maintainer's instruction on the planning page; it runs once the maintainer unholds it.

Stated on the planning page and not objected to: treating the image types as safe also removes the warning for anyone
who lists them by hand, and a saved list is left alone.

## Requirement sources

**The user's request and decisions:** the TODO entry and Decisions above.

**Binding repository constraints:** SPEC.md's two paragraphs above; root `AGENTS.md`, "Tests and validation", "Changelog
fragments", "Conventional Commits"; `website/AGENTS.md` before editing the docs page (an incidental docs edit in an
unattended run starts no preview server; name the changed page in the report).

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Change only `DEFAULT`, `warning_kind` and the tests and text that name them; no stored-preference migration, since an
  unset preference already reads `DEFAULT`, and the settings draft stores it only on an explicit save. Check that
  nothing else stores the default's text.
- The test for criterion 2 is a pure one in `open_extensions.rs`: every entry of `DEFAULT` is `Safe`, and `svg` stays
  `Html`. If you run the settings tests at all, run them with `--features desktop`.
- Changelog kind `changed`: Cmd-click now also opens these image files in their default app.

## Outline

One PR: `plan/cmd-click-images/01-image-defaults`, `feat:` with a changelog fragment and the TODO entry removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
the changed crates, the unit tests of `farhelm-proto`, `farhelm-helm`'s downloads module and, with `--features desktop`,
`farhelm-ui`'s settings module, through `python3 scripts/record-test-run.py`, `dprint check` on changed Markdown, and
`python3 releasing/check-changelog.py format`.

## Fallbacks

If another plan has moved the code this file names, follow the current code and correct this file's names and line
numbers to the truth, logged as a DECISION. If main already has these types in the default, the plan is done except for
whatever criteria remain; finish those.

Examples of departures that need the scope reassessment: a migration of saved preferences, a new warning class for
images, or changing how the helm checks a file at open time.

Decisions to log: the changelog kind, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-cmd-click-images-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
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
available as a native sub-agent type, `$scode-harness-shellout` supplies the launch mechanics.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/cmd-click-images/<nn>-<short-name>`, as
  the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- The plan's last code PR removes this plan's TODO.md entry.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`) at high effort.
Launch it as a native sub-agent with that model and effort when your harness serves it, and otherwise through
`$scode-harness-shellout`. If it cannot be reached when a review is due and is still unreachable after a reasonable
retry, block the plan per `plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is
consistent. Never substitute another reviewer.

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

The plan is complete when a linear stack of draft PRs exists in the order the outline gives, together meeting the
acceptance criteria above (as refined by Decisions already made), each having passed its review gate, with the last one
removing this plan's TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be
satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its
report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
