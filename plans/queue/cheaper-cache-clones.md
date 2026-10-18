# Managed checkouts: clone straight from the host's repository cache

Written against main at 63439c2f on 2026-10-17. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Make a managed checkout's clone stage cheap by cloning straight from the host's repository cache (hardlinked pack files,
no repacking) and then fixing the result up so it is indistinguishable from a plain clone from GitHub. This "optimized"
clone is the default. A new global setting turns it off, and then the clone stage runs exactly today's clone
(`--reference <cache> --dissociate` from GitHub), which becomes the "non-optimized" behavior.

TODO.md entry this plan covers (Near term): "**Cheaper clones from the repository cache.** Look into whether a managed
checkout's clone from the host's repository cache can skip most of the compression and delta work git does today, so the
clone stage finishes sooner."

Why it is slow today: `clone_with_repo_cache` in `crates/farhelm-supervisor/src/launch.rs` runs
`git -c core.commitGraph=false clone --reference <cache> --dissociate -- <https url> <cwd>`. The `--dissociate` step
repacks every borrowed object into a fresh pack for the checkout, delta search included. Measured during planning on git
2.43 (clone step only, the optimized figure including its 0.3s default-branch lookup): git.git takes 36s as a plain
clone from GitHub, 16s with today's command, and about 1.3s optimized; tokio-rs/axum 1.4s, 0.6s and 0.4s.

### Acceptance criteria

- With the setting on (the default, including for existing installations), the clone stage runs, all while holding the
  cache's repository lock exactly as today:
  1. `git ls-remote --symref <https url> HEAD`, parsing the `ref: refs/heads/<X>\tHEAD` line for the default branch. It
     runs before the cache fetch, so a default branch that just changed is still covered by that fetch. `<X>` is network
     input that becomes an argv element and a path: check it with `git check-ref-format` before using it.
  2. Today's `git init --bare -q <cache>` and cache fetch, unchanged.
  3. Routing (see below), then `git --git-dir <cache> symbolic-ref HEAD refs/heads/<X>`, so the clone tracks the default
     branch. (A bare cache's HEAD otherwise follows `init.defaultBranch`; without this step a repository whose default
     branch is anything else clones with nothing checked out, no `<remote>/HEAD`, no upstream, and a `branch.master`
     config.)
  4. `git -c protocol.file.allow=always clone --no-checkout -- <cache> <cwd>`, without `--local`, so git hardlinks pack
     files on the same filesystem and copies them across filesystems. The `-c` keeps a user's
     `protocol.file.allow=never` or `protocol.allow=never` from refusing this one local clone. `--no-checkout` keeps
     every reader of the remote URL (Git LFS, smudge filters, a user's `post-checkout` hook) from running while the
     remote still points at the cache.
  5. Fix-ups: `git -C <cwd> remote set-url <remote> <https url>`, where `<remote>` is read from the clone
     (`git remote`), because git honors the user's `clone.defaultRemoteName`; rewrite the `clone: from <cache path>`
     reflog message to `clone: from <https url>` in `.git/logs/HEAD`, `.git/logs/refs/heads/<X>` and
     `.git/logs/refs/remotes/<remote>/HEAD`, failing the clone stage if any of the three is missing or lacks that line
     (so an unexpected ref backend fails closed); unlink any `.keep` files the clone copied into `.git/objects/pack/` (a
     plain clone has none, and one pins its pack); then fail the clone stage if any file under `.git` outside
     `objects/pack/` still contains the cache path, or if `objects/info/alternates` exists.
  6. Check out, with the remote already pointing at GitHub, in a form that runs the same smudge filters and the same
     `post-checkout` invocation a normal clone's checkout runs (clone passes `<null sha> <head sha> 1`) and adds no
     reflog entry a plain clone lacks. Planner proposal, to verify: populate the index and work tree from HEAD with
     `git read-tree -u -m HEAD` (or the equivalent plumbing), then
     `git hook run post-checkout -- <null sha> <head sha> 1` (git 2.36+). If the host's git lacks what this needs, that
     checkout takes today's clone (routing rule 3 below). The same environment scrubbing (`REPOSITORY_ENV_VARS`), launch
     markers and `GIT_TERMINAL_PROMPT=1` apply to every command. `ls-remote` needs its stdout captured while stdin and
     stderr stay on the session terminal, so a credential prompt still shows.
- Routing, decided before any clone runs, and these exactly: today's clone is used for that checkout when (1)
  `ls-remote` gives no default-branch line (an empty repository with an unborn or unadvertised HEAD); (2)
  `refs/heads/<X>` is missing from the cache after the fetch (the default branch was renamed or deleted between the two
  commands); (3) the host's git cannot perform step 6 exactly; (4) the cache uses the reftable ref backend
  (`git --git-dir <cache> rev-parse --show-ref-format` reports `reftable`; an older git without the option means files);
  (5) only if the equivalence test shows step 6 cannot reach equivalence for Git LFS: the default branch's tree has a
  `.gitattributes` assigning `filter=lfs`. Each routed shape is logged as a DECISION and named in SPEC_impl.md, docs and
  the report. Any other failure fails the clone stage as today, with an error that names the Settings switch. A new
  routed shape is a scope change.
- Result, for a freshly filled cache: compared with a plain `git clone <https url>` of the same upstream, every file
  under `.git` except `index` and the object store is byte-identical apart from reflog timestamps (`config`, `HEAD`,
  `packed-refs` with peeled lines, the loose `<remote>/HEAD` symref, reflogs, hooks, `description`, `info/exclude`);
  `git ls-files --stage` matches and `git status --porcelain` is clean in both; the object sets match; the same
  `post-checkout` arguments and smudge filter calls occur, seeing the GitHub URL; no `FETCH_HEAD`, no alternates. The
  cache's own config and hooks are not copied.
- The one accepted difference, from a long-lived cache: objects only the cache still holds (for example from a branch
  since deleted upstream) arrive unreachable, and the pack layout and git's own index files (`commit-graph`, bitmap,
  `objects/info/packs`, a cruft pack's `.mtimes`) follow the cache's. `git fsck` reports such objects as dangling; git's
  own cleanup removes them once they are older than its prune window (`gc.pruneExpire`, two weeks by default; hardlinked
  packs keep the cache's age). SPEC.md and the user docs state this difference plainly. The cache is not pruned to
  shrink it.
- With the setting off, or for a routed checkout, the clone stage runs today's three commands unchanged, including the
  `core.commitGraph=false` workaround, which belongs to the non-optimized path only. A routed checkout makes three
  network requests (lookup, fetch, clone), so it can ask for credentials up to three times; the docs say so.
- The setting is global (every host), stored by the helm as a column next to `claude_trust` in `checkout_config`, shown
  as a switch with a one-line note in Settings → managed checkouts, on by default, with a GET/PUT route. Mirror only the
  stored boolean from the Claude trust opt-in: no hint band, no dismissal preference, no CLI flag, and no
  `config_revision` bump on write (as `set_claude_trust`). The value is frozen at create into
  `farhelm_proto::ResolvedGithubCheckout` (`crates/farhelm-proto/src/github_checkout.rs`, set in
  `crates/farhelm-helm/src/sessions.rs`, read in `crates/farhelm-supervisor/src/service/core.rs`) and from there into
  the supervisor's `CheckoutPreparation`. Use the same encoding as `claude_trust` there,
  `#[serde(default, skip_serializing_if = "std::ops::Not::not")]`, with `false` meaning today's clone:
  `ResolvedGithubCheckout` is part of the create fingerprint, so an always-present key would turn retries across the
  upgrade into refused key reuse, and stored pre-upgrade snapshots must replay unchanged as non-optimized.
- Every checkout stays independent: deleting the cache leaves the checkout passing `git fsck --full`.
- SPEC.md (the managed checkouts paragraph around "clones from GitHub using its cached objects", ~line 919),
  SPEC_impl.md (the repository cache commands, ~lines 2048-2066), `docs/github-checkouts.md` ("Repository caches") and
  `website/src/content/docs/docs/using/start-a-session.mdx` (the cache paragraphs) describe both behaviors, the setting,
  the routed shapes, and the stated difference. "A private repository without a credential helper can ask twice" stays
  true for the optimized path (the lookup is its second network request, as today's clone is).
- The upgrade path holds (SPEC.md, "Upgrade compatibility and client scale"): the new column defaults to on for existing
  databases, and a newer helm with an older supervisor, or the reverse, keeps working (the older side runs today's
  clone).

## Requirement sources

**The user's decisions (2026-10-17):**

- The bar, in their words: "If we can reliably promise that after a local clone and rewriting the remotes, it truly
  looks exactly the same as if this never happened, I want there to be no chance of some difference that breaks some
  tool that somebody's using. This needs to be transparent if we do it. But from a performance standpoint, I really like
  the hardlinks to the pack files and so on and making it truly cheap."
- After seeing the experiment (equivalent for a fresh cache; leftover unreachable objects and pack layout from a
  long-lived cache, invisible to ordinary tools): go ahead, optimized on by default, with that difference stated in the
  spec and docs. Do not prune the cache to shrink it.
- A global setting turns the optimized behavior off. Their latest word on what "off" means, reversing an earlier answer
  that wanted a plain clone from GitHub: "Can we make the current behavior the non-optimized, and then what we're
  designing here is the default on, but able to turn off? ... I want the behavior when turned off to still be better
  than literally cloning fully from remote every time." So there are two behaviors only: optimized (new) and today's.
- Hardlinks are fine.
- Leftover unreachable objects from a long-lived cache may stay in a checkout until git's prune window expires (about
  two weeks by default; hardlinked packs keep the cache's age). Acceptable, stated that way in the spec and docs.
- The new helm setting's schema change leaves no downgrade path; the user accepts that.
- Review gate: a fresh-context gpt-6.1-sol agent at high effort. No-workhorse mode.

**Binding repository constraints:** SPEC.md's managed checkouts paragraph (constructed HTTPS URL, user's own
credentials, cache refresh then clone in the session terminal, concurrent checkouts take turns, every checkout
independent of the cache, origin stays the GitHub URL, a cache failure fails the clone stage with the cache path and no
fallback). SPEC_impl.md's repository cache section (lock order, CLOEXEC descriptors, `safe.bareRepository=explicit`
kept, foreground gc, `mark_used` after success, the startup sweep). SPEC.md "Upgrade compatibility and client scale".
Root `AGENTS.md` (Tests and validation, Changelog fragments), and `.agents/test-authoring.md` (no test changes the test
process's own environment; the d2 tests inject git config on the child command).

**Planner proposals (checked by a fresh-context planning review):**

- The command sequence and checks under Acceptance criteria come from a hands-on comparison during planning (git 2.43;
  tokio-rs/axum, petervanderdoes/gitflow-avh whose default branch is `develop`, and git/git): byte-identical `.git`
  apart from timestamps and the index's stat data for a fresh cache, with the stated difference for a simulated
  long-lived cache. Not covered there, and added after the planning review: `--no-checkout` plus a separate checkout
  (because Git LFS and hooks would otherwise see the cache path as the remote during the clone's own checkout), `.keep`
  removal, the reftable and branch-race routing, and the ref-name check.
- The checkout form in step 6 is a proposal to verify, not a settled mechanism; the requirement is the observable
  equivalence (same filters, same `post-checkout` arguments, no extra reflog entry).
- The five routing rules are the complete list; the global switch is the user's escape hatch for anything else.
- Do the reflog rewrite, `.keep` removal and the cache-path scan in Rust inside the shim, not with shell tools.
- Switch wording in product terms (for example "clone new checkouts from this host's repository cache"), with a note
  that a long-lived cache can leave unused objects in a new checkout until git's own cleanup removes them. Final wording
  is yours, per root `AGENTS.md` (Talking to the user).

## Outline

Two PRs:

1. `perf`: the optimized clone in the supervisor, selected by the new preparation field (no helm side yet, so nothing
   changes for users): the sequence, fix-ups, checkout, routing, the argv test for both paths, the no-fallback test on
   the optimized path, the real-git equivalence test, and the SPEC_impl.md commands. Its changelog fragment is
   `kind: none` with the reason that the path is unreachable until the setting lands.
2. `perf`: the helm column (schema bump, default on), its route and the Settings switch, freezing it into
   `ResolvedGithubCheckout`, the SPEC.md paragraph, the docs, and the TODO.md removal. It edits PR 1's fragment into the
   user-facing entry (`kind: changed`): new checkouts clone much faster, the switch, and the stated difference.

### Validation

Choose per `.agents/validation.md`; at least:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`.
- Through the recorder: the supervisor's `launch::` tests and the `repo_cache` sweep tests, the helm's `checkout_config`
  tests including a schema-upgrade test for the new column, `ResolvedGithubCheckout`'s encoding test extended to the new
  field, and `crates/farhelm/tests/e2e/github_checkouts.rs`. Optimized-path coverage goes where it says something new:
  `d1_fake_git_hook_agent_run_in_order_with_exact_argv` on both paths (`fake_git_script` needs stages for `ls-remote`
  and `git remote` with stdout, `symbolic-ref`, `remote set-url`, the checkout, and a clone that writes the reflog
  files), `cache_git_failure_has_no_plain_clone_fallback` on the optimized path (a failure must not drop into today's
  clone), and every `d2_real_git_*` on the optimized path.
  `missing_cache_field_refuses_clone_but_ready_restart_still_works` and
  `concurrent_checkouts_serialize_cache_fetch_and_copy` stay on one path.
- The new equivalence test, real git, no network: an upstream bare fixture with several branches, lightweight and
  annotated tags, a default branch other than `main` or `master`, and a `.gitattributes` `filter=probe` entry, mapped
  from the constructed HTTPS URL with `url.<file:// upstream>.insteadOf` injected on the child command as the d2 tests
  do (`file://` makes the reference clone use the pack protocol, like a network clone). Inject on the child commands
  `filter.probe.smudge`/`filter.probe.required` and a `core.hooksPath` `post-checkout` probe, each recording its
  arguments and `git config remote.<remote>.url`. Compare a reference `git clone <https url>` against the optimized path
  as in the Result criterion, including the probes' records. Cover a user `clone.defaultRemoteName`, a user
  `protocol.file.allow=never`, the long-lived case (a branch deleted upstream after the cache saw it; only dangling
  objects differ), routing rules 1 and 2, and cache deletion followed by `git fsck --full`. Configure git per child
  command, never in the test process's environment.
- Git LFS when `git-lfs` is available (you may install it in your scratch directory, or skip with a logged reason): an
  LFS-tracked file in the fixture, served from a local LFS endpoint if feasible, compared the same way. If it cannot be
  exercised, say so in the report and keep routing rule 5 out unless evidence says it is needed.
- By hand, recorded in the report: reftable routing if the host's git supports it; a cache and a checkout on different
  filesystems (copy instead of hardlink) where one is available; a timing of the clone stage on a large public
  repository for both paths.
- If the Settings switch is added to a browser spec, that spec on Chromium and WebKit through the recorder. Not the full
  suite.
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`,
  `python3 releasing/check-changelog.py format`, `dprint check` on changed Markdown, and the website build if a docs
  page changed.

## Fallbacks

- The five routing rules above, and no others.
- If the equivalence bar cannot be met in a general case outside those rules, stop building the optimized path and block
  with the concrete difference as the question; the user set the bar ("no chance of some difference").

Examples of departures that need the scope reassessment: pruning or repacking the cache, a third clone behavior, a
per-host override of the setting, a hint band or CLI for the setting, copying instead of hardlinking on purpose,
rewriting objects in the checkout, probing for further shapes to route.

Decisions to log: the field and setting names and wording, the checkout form chosen for step 6 and its evidence, which
routing rules were implemented (rule 5 only with evidence), the measured timings.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-cheaper-cache-clones-log.md` in the parent directory of the checkout you run in, derived as that section
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
  `plan/cheaper-cache-clones/<nn>-<short-name>` (for example `plan/cheaper-cache-clones/01-optimized-clone`).
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
