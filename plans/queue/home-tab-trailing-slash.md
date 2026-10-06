# Make a terminal tab beside a session in `~` start in `~`, not `~/`

Written against main at b3003cef on 2026-10-05. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out this TODO.md entry (Near term bucket), and remove it in the same PR:

> **Terminal tab next to an agent started in `~` opens in `~/`.** Opening a terminal tab beside an agent session whose
> folder is the home directory gives a shell whose prompt shows `~/` rather than `~`, so its working directory carries a
> trailing slash. Find where the slash comes from and make the tab start in exactly the session's folder.

Acceptance criteria:

- A terminal tab opened beside a session whose stored folder ends in `/` (for example `/home/u/`) starts a shell whose
  `PWD` has no trailing slash, so the prompt shows `~` for the home directory. `/` itself stays `/`.
- This holds for sessions created before the fix too, without rewriting any stored session data.
- The working log and the report say where the slash came from, as far as it can be established (see the outline).
- One draft PR, `fix:`, with a changelog fragment (`kind: fixed`), reviewed per the review gate, removing the TODO
  entry.

## Requirement sources

**The user's request (2026-10-05):** "lets plan 'trailing slash in ~', 'cmd-N for new session', 'installer feedback
prompt' - a group of easy ones", planned as three separate plans at the user's choice; this is one.

**The user's plan-time decisions (2026-10-05):** review gate "opus 5.5 and astra high (no swarm)" (see Review gate);
no-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** are the outline below. A fresh-context planning review checked it against the code on b3003cef.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Releases and the changelog;
Sharing the machine with other agents; Agent scratch space; The live install is off-limits), `plans/AGENTS.md`
(Executing one plan), and `.agents/test-authoring.md` for test changes.

## Outline

Line numbers drift; find the code by name. Verified on b3003cef:

- A terminal tab is opened by the supervisor with tmux `new-window -c <start dir>`, where the start directory is the
  session's stored folder passed through `tmux_start_directory` (`crates/farhelm-supervisor/src/tmux.rs`; the caller is
  `open_tab_window` in `crates/farhelm-supervisor/src/service/core.rs`). `tmux_start_directory` today only escapes `#`.
  The agent's own window (`create_session`) and restarts (`respawn-pane`) go through the same function.
- tmux sets the child's `PWD` to exactly the `-c` string. A shell that trusts an inherited `PWD` naming its current
  directory (zsh, the macOS default) keeps a trailing slash and shows `~/`. Bash likely canonicalizes it; that part is
  unverified and does not matter for the fix.
- Ways a stored folder can end in `/`: `expand_tilde_cwd` (`service/core.rs`) returns the supervisor's home verbatim for
  `~` and `~/`, so a host whose `HOME` ends in `/` stores one; a typed absolute folder with a trailing slash is stored
  as typed; and `farhelm spawn --cwd ~/` (or any agent or CLI create) arrives already expanded by the caller's shell to
  `/Users/x/` and is forwarded verbatim. The launcher's default `~` and the folder picker cannot produce one on an
  ordinary host.

The fix: strip trailing slashes from the start directory once, in `tmux_start_directory` (keeping `/` as `/`, and `//`
becoming `/`), so every window Farhelm starts (tabs, the agent window, restarts) gets a clean `PWD`. That covers new and
existing sessions and changes no stored data.

Do NOT:

- normalize in `expand_tilde_cwd` or anywhere on the create path. It would change the documented rule that `~` expansion
  is the only rewrite of the caller's spelling, could make an interrupted create that is retried after an upgrade
  compare unequal to its recorded row and be refused, and would not fix existing sessions;
- open tabs at the session's canonical (symlink-resolved) folder: SPEC.md's session view rules keep the user's spelling
  with ordinary resolution.

Where the slash came from in the maintainer's case cannot be settled from here (the live install is off-limits). Record
the candidate sources above in the log and the report, say which ones the fix covers (all of them), and do not spend
effort reproducing the maintainer's environment.

Regression: a unit test on the normalization (`/x/` → `/x`, `/` → `/`, `//` → `/`, a path with no trailing slash
unchanged, and that `#` escaping still applies). If one of the existing scratch tmux-server tests in `tmux.rs` can be
extended in a few lines to show a new window's shell sees `PWD` without the slash, add that as the end-to-end proof;
otherwise the unit test is enough, and say so in the report.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression. Typical choices, not
a checklist: `cargo fmt --all -- --check`, clippy on `farhelm-supervisor`, and a focused nextest selection for the
touched `tmux` tests through `scripts/record-test-run.py` (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Because the agent window and restarts share the function, include the existing
start-directory tests (the `C#Samples` escaping ones) in the selection. Run `python -B scripts/check-test-sleeps.py` per
`docs/test-sleep-check.md` and apply `.agents/test-authoring.md` for test changes.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-home-tab-trailing-slash-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/home-tab-trailing-slash/<nn>-<short-name>`.
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
