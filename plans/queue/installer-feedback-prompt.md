# Ask for feedback when a fresh install finishes

Written against main at b3003cef on 2026-10-05. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out this TODO.md entry (Near term bucket), and remove it in the same PR:

> **Ask for feedback when the installer finishes.** The installer's closing message should directly encourage the user
> to send feedback through the `?` menu's Send feedback, say that it goes privately to the maintainer, and say that even
> a low-effort, throwaway comment is useful. The installer tests check its exact closing messages, so they change with
> it.

Acceptance criteria:

- A successful FRESH install (`scripts/install.sh`, `updated_installation=0`) prints the agreed text below, verbatim, in
  every tmux variant. An update prints nothing new.
- The installer tests assert the new exact texts and still pass.
- One draft PR, `feat:`, with a changelog fragment, reviewed per the review gate, removing the TODO entry.

## Requirement sources

**The user's request (2026-10-05):** "lets plan 'trailing slash in ~', 'cmd-N for new session', 'installer feedback
prompt' - a group of easy ones", planned as three separate plans at the user's choice; this is one. Then: "lets agree on
the 'feedback prompt' exact wording etc during planning".

**The user's plan-time decisions (2026-10-05):**

- Fresh installs only, not updates. (Updates are mostly the app's own background runs, whose output nobody sees.)
- The exact text, chosen from drafts as "option C except let's also say something like 'Alternatively if you prefer you
  can file a public github issue at https://github.com/scode/farhelm'", then picked in this final form:

  ```
  💬 I'd love to hear what you think, even a quick throwaway
     comment. In Farhelm, click ? at the top of the sidebar and
     choose Send feedback; it comes privately to me, the maintainer.
     If you'd rather discuss it in the open, file a GitHub issue
     at https://github.com/scode/farhelm/issues instead.
  ```

  The line breaks and the three-space continuation indent are part of the agreed text. Do not reword it; a change is a
  question for the user (Unattended fallback).
- Placement, as previewed to the user: after "Open Farhelm from Spotlight or ~/Applications." and before "To uninstall
  later, run: …", separated by blank lines like the other blocks; the tmux warning block stays last.
- Review gate "opus 5.5 and astra high (no swarm)" (see Review gate); no-workhorse mode, which `plans/AGENTS.md`
  requires for every plan.

**Planner proposals** are the remaining details in the outline. A fresh-context planning review checked them against the
code on b3003cef.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work, including the installer
checks; Releases and the changelog; The live install is off-limits: never run the installer against this machine's real
home), `plans/AGENTS.md` (Executing one plan), and `.agents/test-authoring.md` for test changes.

## Outline

Line numbers drift; find the code by name. Verified on b3003cef:

- `scripts/install.sh`'s closing report: the status line (`✅ Farhelm X is installed.` or `… is ready.` for an update);
  for a fresh install with tmux at the floor, the "Open Farhelm from Spotlight or ~/Applications." line; then the
  kept-file notice (`KEPT_NOTES`, printed when a pre-existing `~/.local/bin` file was renamed aside); then the uninstall
  line; then, when tmux is missing or below the floor, the tmux warning block.

Planner decisions on the details:

- The feedback block goes immediately after the "Open Farhelm…" line, before the kept-file notice when there is one, so
  the call to action sits with the open instruction and the housekeeping notices follow. When tmux is below the floor,
  the "Open Farhelm…" line is not printed and the block follows the status line directly; the tmux warning still ends
  the report, so the invitation precedes "Then open Farhelm…". That ordering is the agreed placement, not something to
  reopen.
- Styling: the URL in `OUT_CYAN`/`OUT_RESET` on a terminal, the way commands are colored; plain text when output is
  redirected or `NO_COLOR` applies, as the existing color variables already handle. No OSC 8 hyperlink (unlike
  `BREW_LINK`): terminals already make a plain URL clickable, and a second hyperlink variable is machinery nobody asked
  for. Everything else in the block is unstyled.
- `scripts/test-install-sh.sh`: five exact-text expectations change, not six. The three FRESH variants of
  `assert_closing_message_contract` (tmux at floor, absent, old) gain the block; the three update variants stay
  unchanged. `K1_EXPECTED` (fresh install with a kept-file notice) gains it, which is where the before-the-notice
  placement shows. The terminal-output Python block's `expected` (fresh install, absent tmux, compared after stripping
  styling) gains it too. Add one check that an update's report does not contain the feedback text, if the existing
  update-variant exact matches do not already make that obvious to a reader.
- No SPEC.md or website change: SPEC.md does not describe the closing message's contents, and the docs site's install
  page ("finishes by telling you Farhelm is installed and how to uninstall it later") stays true.
- Changelog: `feat:` (the installer's output changes for the user); choose the fragment kind (`added` or `changed`) and
  log it as a DECISION.

### Validation

`sh -n scripts/install.sh && shellcheck scripts/install.sh scripts/test-install-sh.sh`, and
`bash scripts/test-install-sh.sh` (root `AGENTS.md` describes what it covers; it runs the installer against fixture
homes and a fixture server, never this machine's real installation). No Rust, browser, or macOS uninstall suite: nothing
they cover changes. Apply `.agents/test-authoring.md` to the test edits.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-installer-feedback-prompt-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs)
rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on, or
that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
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
  `plan/installer-feedback-prompt/<nn>-<short-name>`.
- This plan is one PR. If the work turns out to want more than one, split it into a linear stack of bite-sized PRs
  without churn (nothing added in one PR and removed in a later one), and log the split as a DECISION. Within this run,
  if a PR needs correcting, restructure it rather than stacking a correction on top.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect. A `feat:` or
  `fix:` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases
  and the changelog), written for someone running Farhelm per `releasing/EDITORIAL_GUIDANCE.md`. Validate with
  `python3 releasing/check-changelog.py format`.
- The PR removes this plan's TODO.md entry (named under The goal) in the same commit. It never touches `plans/`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; the planning system's monitor lands the plan.

### Review gate

Before finishing each PR that changes code, scripts or tests, use the active galaxy-brain skill to delegate two
independent reviews of that PR's changes, and address what both find before moving on. The user demands exactly these
reviewers, and no review swarm ("opus 5.5 and astra high (no swarm)"):

- a fresh-context agent on Opus 5.5 at high effort;
- a fresh-context agent on gpt-6-astra at high effort, shelled out to the harness that serves that model when the
  executing one cannot reach it natively.

Both get the same prompt, carrying the full charter, because neither reviewer has anything else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a separate
findings file per reviewer (in the scratch directory), and the acceptance criteria: The goal and the user's decisions
from this file, quoted. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the merits and log the DECISION. Do
not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (examples, not a blacklist: a new shared abstraction,
a new persisted field, a change to a protocol message, a JS-to-Rust event channel, a rewrite of a flow rather than a
small change in it), and whenever the same component has needed repeated corrective review rounds, run a fresh-context
review through galaxy-brain with this charter, supplying the user's request and decisions, this outline, the current
diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the Conventional Commit type and changelog kind, anything the outline left to
you, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the user's requirements still hold. A material scope expansion, a weakened guarantee, an omitted
required behavior, or a change to text the user agreed verbatim needs the user's decision; a review finding or a log
entry is not authorization. If the work needs such a decision, record the concrete tradeoff and block per
`plans/AGENTS.md` (Executing one plan, step 10).

## Done criterion

The plan is complete when its draft PR (or linear stack, if split) exists, does what The goal says, has passed the
review gate, and removes this plan's TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest
entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver
its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/`
yourself.
