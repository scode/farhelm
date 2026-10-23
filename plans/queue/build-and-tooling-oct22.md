# Installer, release build and maintainer tooling fixes

Written against main at c3345af7 on 2026-10-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Five fixes in scripts: the Linux private tmux build gets a fixed terminal-definition search path instead of the build's
deleted `/tmp` folder; the plans queue script ignores pull requests from forks; the CentOS provisioning test refuses to
rewrite the user's ssh config when a marker block has no end; a rerun installer replaces a damaged version folder
instead of keeping it; devbuild verifies the release signature of the tmux it pushes to remote hosts.

Carry out these 5 triage outcomes, each recorded under its own heading in `TRIAGE_OUTCOMES.md`, in this order (ledger
order, which `review_feedback_queue/.agents/execute-outcomes.md` asks for), one PR each. Read every entry in full first.
Each has a size gate of about 100 lines; over it, block and bring it back.

1. `private-tmux-terminfo-path-under-tmp.md` (`fix code`)
2. `plans-queue-counts-fork-prs-as-plan-work.md` (`fix code`)
3. `centos-ssh-scrub-drops-config-after-unmatched-marker.md` (`fix code`)
4. `power-loss-version-folder-never-repaired.md` (`fix code`)
5. `devbuild-tmux-skips-release-signature.md` (`fix code`)

## Outline

Facts from the triage session's reading of main at b97bcfe4 (verify them; line numbers drift). Before touching
`scripts/`, read `.agents/validation.md` (root `AGENTS.md`, "Instructions by path").

- PR 1: `scripts/build-private-tmux.sh` creates its work folder with `mktemp -d "${TMPDIR:-/tmp}/farhelm-tmux.XXXXXX"`
  (around 18) and configures ncurses with only `--prefix` for the Linux musl targets (around 88-97, 126), so the only
  system terminfo directory compiled in is under that deleted folder; the macOS branch (around 109-115) already sets
  `--with-default-terminfo-dir=/usr/share/terminfo`. Give the Linux build a fixed system search list (for example
  `/etc/terminfo:/lib/terminfo:/usr/share/terminfo`, with `--disable-db-install` if the database is not needed). Build
  it the way `scripts/build-pinned-tmux-ci.sh` sets the build up (it uses zig, so it runs on this machine), with its
  output in your scratch directory, never `.ci-tmux/`, and check with `strings` that no `/tmp` path remains; the ledger
  requires that check, so if the build cannot run, block rather than skip it. This changes release tmux bytes; mention
  it in the PR.
- PR 2: `scripts/plans-queue.py`: `GitHub.open_pulls` and `GitHub.pull` (around 759-772) keep only `.head.ref`, and
  `merged_pulls` (around 773-780) searches `head:<prefix>`; every plan association is a bare branch-prefix test
  (`status` around 1280-1285, `_require_merged` around 878-897, `abandon` around 1010-1030). Capture
  `head.repo.full_name` and ignore PRs whose head repository is not this repository. Add a cross-repository PR to the
  fake GitHub in `scripts/test-plans-queue.py` (around 205-218) and show `status`, `landed` and `abandon` ignore it.
  This is a code PR: it never touches `plans/` (`plans/AGENTS.md`, "Editing rules").
- PR 3: `scripts/test-provision-centos.sh`: both awk scrubs (around 177-181 and 357-361) drop every line after a start
  marker with no end marker before `replace_ssh_config` (around 148-158) publishes the result over the real
  `~/.ssh/config`. Make both fail (for example `END { if (skip) exit 3 }`) and leave the config untouched with a
  message; the existing `|| exit 1` paths already avoid publishing. Never run the whole CentOS script for this: it
  rewrites the real `~/.ssh/config`. Test only an extracted awk program or the sweep against a scratch `HOME` if the
  script's structure allows it; otherwise reason it from the code and say so.
- PR 4: `scripts/install.sh` keeps an existing version folder whenever its `farhelm` exists (around 1259-1265, comment
  "a release's bytes do not change"); the staged copy is checksum-verified (around 1037-1045) and `same_bytes` exists
  (around 391-396). Keep it only when the bytes match, otherwise replace it as an incomplete folder is replaced; update
  the comment. Add a case to `scripts/test-install-sh.sh` that truncates the version file and reruns; the existing
  same-release rerun case (around 798) still passes. Never run the installer against the live install (root
  `AGENTS.md`); the test harness uses private fixture homes.
- PR 5: `scripts/devbuild-mac.sh` (around 561-605) fetches `SHA256SUMS` and the tmux, never `SHA256SUMS.minisig`, and
  reuses the cached copy (around 565). The helm's verifier is `farhelm_helm::verify_signed_sums`
  (`crates/farhelm-helm/src/provisioning/release_payloads.rs` around 990) with `RELEASE_KEY_RING` (around 97); the
  `farhelm` CLI depends on the helm crate, has hidden internal subcommands (`crates/farhelm/src/main.rs` around 1239),
  and is built before the tmux step (`devbuild-mac.sh` around 475). Add a hidden subcommand that verifies a `SHA256SUMS`
  against its `.minisig` with that verifier, have the script call it before trusting the tmux, and refuse on a bad or
  missing signature; re-check or re-fetch a cached copy. Correct the header comment (around 38-49).

### Validation

- PR 1: the build script on Linux if the toolchain is available, plus `strings`; otherwise the reason in the report.
  `shellcheck` on the script with no new findings.
- PR 2: `python3 scripts/test-plans-queue.py`.
- PR 3: `shellcheck` on the script, no new findings; a scratch-`HOME` run of the sweep if possible.
- PR 4: `sh -n scripts/install.sh && shellcheck scripts/install.sh scripts/test-install-sh.sh`, and
  `bash scripts/test-install-sh.sh`.
- PR 5: unit test for the new subcommand through the recorder, `cargo fmt --all -- --check` and
  `cargo clippy --all-targets -- -D warnings`; `shellcheck` on `devbuild-mac.sh` if the repo lints it (check
  `.agents/validation.md`); the script itself runs only on a Mac, so describe how it was checked.

When a script or its test changes in a way `.agents/validation.md` or `.agents/run-all-tests.md` describe, update them
in the same PR (root `AGENTS.md`, "Tests and validation"). No full battery.

## Fallbacks

Each outcome's gate is binding: over about 100 lines, block and bring it back. Examples of departures that need the
scope reassessment: flushing files before renames in the installer (that is the separate race-free installer TODO, not
this outcome), adding `minisign` as a new devbuild requirement instead of reusing the helm's verifier, or rejecting fork
PRs anywhere other than the queue script's PR association.

Decisions to log: each PR's measured size; for PR 1 the search list chosen; for PR 5 the subcommand's name and
interface.

## Gates, acceptance and requirement sources

### Size gates

Each outcome's gate, where it has one, is in its `TRIAGE_OUTCOMES.md` entry's Completion criteria and is binding: about
100 lines of diff, or the larger number the entry states (code, tests and docs, not counting `TRIAGE_OUTCOMES.md`, the
queue removal or the changelog fragment). The maintainer prefers size gates to time estimates. Over the gate, block and
bring it back to the maintainer (an entry that says "as above" means this): block per `plans/.agents/executing.md`,
"Executing one plan", step 10, after building and pushing the plan's other outcomes, with a question that describes the
larger fix, its measured size, and asks whether to build it or discard the outcome. Measure and log each outcome's size.
The entry's described fix is the triaging session's proposal; a different fix within the same gate that meets the same
completion criteria is fine, logged as a DECISION.

### Acceptance criteria

- Each outcome's Completion criteria in its `TRIAGE_OUTCOMES.md` entry, including any test it names.
- Each outcome's PR removes its feedback file and `review_feedback_queue/INDEX.md` entry and updates its Execution
  field.
- A `fix` PR follows `AGENTS.md`, "Changelog fragments". A fix to work not yet in a stable release (check whether the
  change being fixed shipped in the latest stable tag) edits that work's existing fragment under
  `releasing/changelog.d/` instead of adding a second one; this file's outline names the fragment where the planning
  session found one. Otherwise it adds a fragment (`kind: fixed`, or `kind: none` with the reason when nothing is
  user-facing). `docs`, `test`, `chore` and bookkeeping PRs carry none.
- Any PR that changes Rust or browser tests or their helpers runs `python -B scripts/check-test-sleeps.py` with the
  isolated interpreter `docs/test-sleep-check.md` prepares (root `AGENTS.md`, "Tests and validation"); a Rust change
  also runs `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings` at least once at the tip.

## Requirement sources

**The user's decisions (2026-10-22):** the maintainer decided each outcome on a triage page that day, after a read-only
verification of every finding against main; each outcome's Decision field records the choice and, where they gave one,
their words. Where a principle question decided several outcomes at once, the Decision field says which. While
scheduling, the maintainer agreed to that day's 40 pending outcomes being scheduled as ten plans grouped by area (this
is `build-and-tooling-oct22.md`) and chose the review gate below: a single fresh-context Opus 5.5 agent at high effort
with the general charter, no swarm. No-workhorse mode, which `plans/.agents/planning.md` requires.

**Binding repository constraints:** the rule each entry's Assessment cites;
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes"; `AGENTS.md`, "Changelog fragments",
"Conventional Commits", "Code rules" and "Tests and validation", with `.agents/validation.md`;
`.agents/test-authoring.md` for any test change; and anything this file's outline names as binding for the plan.

**Planner proposals:** the PR order and the outline above, including the notes a fresh planning review added. A planner
proposal is not a requirement: a smaller design that meets the same completion criteria is fine, logged as a DECISION.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-build-and-tooling-oct22-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs
and the `TRIAGE_OUTCOMES.md` Execution fields) rather than starting over. If it does not exist, this is a fresh start. A
plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/.agents/executing.md`, "Executing one plan" (step 7) before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: the review gate keeps the
model and effort this file demands, and the scope reassessment runs at your own model and effort. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
"Sub-agents this file requires", requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from these rules and run as that file says.

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

- Follow `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes": one reviewable commit, stable
  bookmark and draft PR per outcome (unless this file groups some outcomes into one PR, which the maintainer agreed for
  those cases only), in a single linear stack in the order given under The goal above. Each PR carries its outcome's
  agreed change, the removal of its feedback file and `review_feedback_queue/INDEX.md` entry, and its
  `TRIAGE_OUTCOMES.md` Execution update. Record the jj change ID and bookmark in that Execution field before creating
  the PR, then add the PR's URL (with `/changes` appended) to the same change and push it again; no separate bookkeeping
  PR. Replace the Execution field's current `planned in ...` text rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/build-and-tooling-oct22/<nn>-<short-name>` (for example `plan/build-and-tooling-oct22/01-short-name`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): `fix` for a change users of Farhelm can observe, with a changelog fragment in the same commit; `test`
  for a change only to tests or their fixtures; `docs` for documentation and specification text only; `chore` for
  maintainer tooling and queue bookkeeping. Only a fragment-requiring type adds a fragment.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TRIAGE_OUTCOMES.md`, `review_feedback_queue/`, `TODO.md` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`) at high
effort, launched as a native sub-agent where your harness serves that model and otherwise through
`$scode-harness-shellout` (that skill owns the launch mechanics). The prompt carries the full charter, because the
reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, the outline, and the outcome's full
  `TRIAGE_OUTCOMES.md` entry, including any size gate (ask the reviewer to flag anything beyond the gate).

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
alternatives considered: in particular those named under Fallbacks above, whether each gated outcome met its gate (with
the measured diff size) and why, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above. If current code or specs invalidate a recorded triage decision (for example,
the finding turns out already fixed or wrong), that is a block: `review_feedback_queue/.agents/execute-outcomes.md`,
"Execute triage outcomes", returns such an item to the user rather than changing the agreed outcome.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every outcome that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per outcome listed under The goal unless this file
groups some into one PR (an outcome discarded under its gate still gets its bookkeeping PR; one whose gate says to bring
it back makes the plan block instead, after its other outcomes are pushed), that together meets the acceptance criteria
above, each PR passed its review gate, and each outcome's `TRIAGE_OUTCOMES.md` Execution field names its change ID,
bookmark and PR. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then
close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the
queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Decisions

### 2026-10-23: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-10-23 (claim 56e32e).
>
> ### What this is about
>
> On the 2026-10-22 triage page you chose five fixes for this plan, each to stay within about 100 changed lines or come
> back to you. Four stayed within it and wait as draft PRs: the Linux tmux that Farhelm installs on hosts now finds
> terminal definitions in the system locations (#2657, 10 lines), the plans queue script ignores pull requests from
> forks (#2659, 86 lines), the CentOS provisioning test no longer wipes an ssh config with an unmatched block marker
> (#2660, 36 lines), and rerunning the Mac installer repairs a damaged version (#2662, 39 lines).
>
> The fifth is over the limit. Devbuild is the script that builds a branch on your Mac and runs it as a separate app.
> When you press Update on a remote host in that app, devbuild also gives it the latest stable release's tmux to install
> there. Devbuild checked that tmux only against the release's checksum list, fetched over HTTPS, and never against the
> release signature that the helm requires of its own downloads. It also reused a cached copy on later runs without
> checking it again. So anyone able to change what the download site serves could replace both the tmux and its checksum
> list, and devbuild would install that tmux on every host it updates; a cached tmux changed on disk was installed with
> no check at all. You chose to have devbuild verify the signature, reusing the helm's own verifier through a hidden
> `farhelm` command, and to re-check or re-fetch a cached tmux.
>
> ### What was built
>
> The fix is built and reviewed, and waits as draft PR #2665.
>
> - Devbuild now keeps the release's checksum list and its signature next to the cached tmux. On every run, the
>   `farhelm` it has just built checks the signature with the release keys compiled into it, and devbuild then checks
>   each tmux, cached or freshly downloaded, against the signed checksums.
> - A bad or missing signature, or a tmux that does not match, stops the run and drops the cache, so the next run
>   fetches everything again. A cache from before this change is fetched again too.
> - Devbuild needs nothing new installed: no `minisign`, since the check is the helm's own code.
> - The new command's test passes: a correctly signed file is accepted, and a wrong release version, a missing signature
>   and a tampered file are each refused. Run by hand against the real v0.30.0 release, its files verify and a tampered
>   copy is refused. The script itself only runs on a Mac, so it was checked by reading and with the shell linters, not
>   run end to end.
>
> ### Why it is over the limit
>
> It is 122 changed lines against the limit of about 100: 46 in the devbuild script, 73 for the hidden command, and 3 in
> a test fixture's README. About 27 of those lines are the command's test, its doc comment included, and about 20 more
> are comments across the script and the command, so roughly 75 are the code itself. The review found no materially
> smaller design that still re-checks a cached tmux and refuses a bad or missing signature.
>
> ### The question
>
> Land the devbuild fix as built, or discard it? Nothing in this plan lands until you answer; then the plan finishes
> with either five PRs or four.
>
> - **Land it** (my recommendation). A devbuild then installs a tmux on your hosts only if the release signature vouches
>   for it, the same rule the helm applies to its own downloads. The costs: the hidden command ships in every release
>   from then on and has to keep working for devbuild; devbuild gains a new way to stop, when a release lacks its
>   signature file or is signed with a key that build does not carry; and about 120 lines to maintain.
> - **Discard it.** The plan finishes with the other four, and devbuild keeps trusting the checksum list over HTTPS
>   alone, as before. The fifth PR then only removes the finding from the review queue and records your discard in the
>   triage ledger; a discard means "not worth the time now", so the finding can be raised again later.
> - A smaller version is not on offer: dropping the cached-copy re-check or the test would cut below the fix you chose,
>   and the rest is already as small as the reviewer could find.

The maintainer's answer:

### Answer (2026-10-23): land the devbuild signature check as built

The maintainer's words, verbatim: "3 - unblock and land"

Restatement: land the devbuild fix in draft #2665 as built (about 122 changed lines), including the hidden `farhelm`
verification command, and finish the plan with all five PRs.
