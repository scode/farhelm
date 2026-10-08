# Show Cmd+N in the New button's hover text

Written against main at a38ea524 on 2026-10-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PR changes them as described below.

This plan has no dependency on another plan.

## The goal

In the Mac desktop app, Cmd+N opens the session launcher (New), but nothing on screen says so. Add the shortcut to the
New button's hover text in the Mac desktop app, and only there.

Acceptance criteria:

- In the Mac desktop app, the New button's hover text reads `new session: start an agent or a command on any host (⌘N)`.
  In the web UI and the Linux desktop app it stays `new session: start an agent or a command on any host`.
- The button's accessible name (`aria_label: "new session"`) does not change.
- The hover text is chosen by the same build condition that installs the Cmd+N shortcut, so the two cannot disagree.
- `docs/manual-mac-checklist.md`'s Cmd+N item also checks the hover text.
- The PR removes the TODO.md entry "Show Cmd+N in the New button's hover text."
- One draft PR, which passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, the maintainer's words):** "Show Cmd+N in the New button's hover text. In the Mac desktop
app Cmd+N opens the session launcher, but nothing on screen says so: the New button's hover text reads 'new session:
start an agent or a command on any host'. Add the shortcut to that text in the Mac desktop app (for example '… on any
host (⌘N)'), and only there: the web UI and the Linux desktop app have no such shortcut, since the browser keeps Cmd+N
and Ctrl+N for a new window. Follow-up to the new-session-shortcut plan, at the maintainer's request while reviewing its
report."

**The user's decisions (2026-10-08):**

- D1. The text is the existing hover text with `(⌘N)` appended, in the Mac desktop app only. The maintainer saw this
  proposal and raised no objection.
- D2. Review gate: "gpt-6.1-sol high effort, no swarm for reviewers."
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** are the mechanisms in Outline that the decisions do not fix. Challenge them through Scope
reassessment rather than treating them as requirements.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Releases and the changelog;
Sharing the machine with other agents; Agent scratch space; The live install is off-limits) and `plans/AGENTS.md`
(Executing).

## Outline

Line numbers drift; find the code by name.

### PR 1: the hover text names the shortcut on the Mac desktop

`feat:` with a changelog fragment (a one-line draft: in the Mac desktop app, the New button's hover text now mentions
Cmd+N; pick the fragment's kind and log it as a DECISION).

- The tooltip is the `"data-tooltip"` literal on the `.new-session-button` in `crates/farhelm-ui/src/list/view.rs`.
- The shortcut is installed in `crates/farhelm-ui/src/lib.rs` (`install_new_session_shortcut`), under
  `#[cfg(native_desktop)]` and `cfg!(target_os = "macos")`. The `native_desktop` cfg comes from
  `crates/farhelm-ui/build.rs` and is visible crate-wide; it is true for the desktop feature on a non-wasm target, so
  the condition `all(native_desktop, target_os = "macos")` is true only in the Mac desktop app.
- Choose the hover text with a constant picked by `cfg!(all(native_desktop, target_os = "macos"))` (or an equivalent
  `#[cfg]` pair). Optionally, name that condition once (a small `const` such as "the Mac desktop owns Cmd+N") and use it
  both here and where the shortcut is installed, so the two stay in step; do that only if it stays this small.
- No unit test that merely compares the two literal strings: such a test cannot exercise the build condition, which is
  the only thing that can go wrong, and CI never builds the Mac desktop app. The manual Mac checklist line is the
  evidence for the Mac branch.
- `docs/manual-mac-checklist.md`: extend the Cmd+N item to check that hovering New shows `(⌘N)` in the Mac desktop app,
  and that the web UI on a Mac does not show it.
- SPEC.md's Cmd+N paragraph (under "Desktop window chrome") may gain a short clause that the New button's hover text
  names the shortcut in the Mac desktop app. Add it only if it reads naturally there; log the choice.
- Remove the TODO.md entry.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`,
`cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop` (compiles the
desktop branch; on Linux it compiles the non-Mac text), `dprint check` on changed Markdown, and
`python3 releasing/check-changelog.py format`. No runtime tests: no test asserts this text, and the browser tooltip
coverage test only checks that a tooltip exists. If the executing host is a Mac with the desktop prerequisites,
`cargo check -p farhelm-desktop` there also compiles the Mac branch; otherwise record that it was not compiled locally.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-new-button-shortcut-hint-log.md` in the parent directory of the checkout you run in, derived as that
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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/new-button-shortcut-hint/<nn>-<short-name>`.
- The stack follows Outline. Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack
  deletes. Within this run, if a PR needs correcting, restructure it rather than stacking a correction on top; that
  applies to all of this plan's own open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entry this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code or tests, use the active galaxy-brain skill to delegate a review of that PR's
changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review swarm: a
fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing
one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria for that PR: its part of Outline, the decisions that apply to it, and
the goal's acceptance criteria. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (a new shared platform-detection module, a runtime
platform check in place of the build condition, a test harness for platform-specific UI text; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the user's request and decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the changelog fragment's kind, whether the build condition is shared with the
shortcut installation, and whether SPEC.md gained a clause, and every review finding you decided not to follow. The user
will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code or tests passed the review gate. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
