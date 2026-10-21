# Keep a spare checkout ready so managed-checkout launches skip the slow clone

Written against main at 2897c914 on 2026-10-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file says which of their sentences to change.

This plan has no dependency on another plan. `helm-ingress-types.md` (pending when this was written) changes helm-side
types near the create path; the last PR here touches `ResolvedGithubCheckout` and the helm's create code, so expect a
small rebase if that plan lands first.

## The goal

Launching a session into a managed checkout spends several seconds of local work in the session terminal today: the
clone borrows the host's cached repository objects and then `--dissociate` repacks all of them into the new checkout,
even when nothing changed upstream. Keep one spare checkout of each repository ready on each host, inside the checkout
folder. A launch moves that spare's contents into its freshly allocated checkout folder (a same-filesystem rename per
entry, effectively instant), fetches only what changed upstream since the spare was made, fast-forwards, and then runs
the post-clone command and the agent exactly as today. Afterwards the supervisor builds the next spare in the
background. Whenever no usable spare exists, the launch clones exactly as today. Separately, and first, make Git
consolidate the repository cache sooner, which speeds up today's clone whether or not spares ever ship.

TODO.md entry this plan covers (Near term): "**Keep a spare checkout ready for managed-checkout launches.** Next to each
host's repository cache, keep one spare checkout made ahead of time with the launch's exact clone command. A launch
takes it with a rename, then fetches and checks out only what changed since it was made, and the next spare is built in
the background. That turns the clone stage into roughly rename plus fetch plus a small checkout, instead of today's
several seconds of local work. Not yet planned; the maintainer chose this over a scheduled repack on 2026-10-20 but has
no time to finish planning it now.

Why the clone is slow today (measured on 2026-10-20 against a copy of the live `scode/farhelm` cache, git 2.43, 3 runs
per variant after a warm-up): the cache had 28 packs and 481 loose objects, and of its unique objects 83 MiB were stored
whole (79 MiB of them blobs) against 11 MiB stored as deltas, because each fetched thin pack gets its missing delta
bases appended whole and nothing recompresses them. `--dissociate` then repacks every borrowed object into the new
checkout, and git's own phase timers put most of a 4.8 s clone in that repack's delta search (2.2 s) and pack writing
(1.3 s). Loose objects are a small part: packing them alone saved about 0.25 s, and dropping the ones already in a pack
or adding a multi-pack index saved nothing. A full `git repack -a -d` of the cache (5.5 s, once) brought the clone to
1.85 s against GitHub; `gc --keep-largest-pack` to 3.4 s; `repack --geometric=2` collapsed to one pack here and matched
the full repack. The cheap complement is to pass `-c gc.autoPackLimit=10` (or so) on the existing cache fetch, so git's
own foreground gc consolidates the cache under the cache lock, at the cost of one slower launch now and then. A repack
without that lock is not safe: git's documentation says a gc or repack running while a fetch writes can delete an object
the fetch just added and corrupt the repository.

Decisions this needs: (1) the spare is a plain clone made earlier and then updated, not a clone made at launch time, so
its reflog shows the update, unchanged files keep older modification times, and the user's hooks and checkout filters
(Git LFS, `filter.<name>.process`) ran when it was made and run again only for the update. An earlier cheaper-clones
plan, abandoned on 2026-10-18, required a checkout shown to match a plain GitHub clone; this has to settle whether that
difference is acceptable. (2) Building the spare is a background clone, which SPEC.md does not allow today (the system
never runs VCS operations implicitly, and cache refresh and clone run in the session terminal so progress, credential
prompts and failures are visible): decide its environment and credentials, and that a failed spare just falls back to
today's visible clone. (3) A rename is only instant on one filesystem, so the spare lives inside the configured checkout
folder, with a normal clone when that is not possible. Also: two launches at once (one takes the spare, the other clones
normally), building the spare in a temporary directory and moving it into place when done, and discarding spares when
the cache is evicted, the checkout folder changes, or the post-clone command changes."

All three decisions are settled below.

### Acceptance criteria

**Cache consolidation (first PR, stands alone):**

- The existing cache fetch passes `-c gc.autoPackLimit=10` alongside its current
  `-c gc.autoDetach=false -c maintenance.autoDetach=false`, so Git's own foreground gc consolidates the cache while the
  cache lock is held. Keep the new `-c` after the existing pair so the test fixture's stage dispatch (`fake_git_script`
  keys on the first two argv words) still recognizes the fetch.
- SPEC_impl.md's repository-cache section and `docs/github-checkouts.md` ("Repository caches") show the new command and
  say that an occasional launch is slower while the cache consolidates.

**Where a spare lives and what it is:**

- One spare per host and repository, in a reserved hidden folder inside the configured checkout folder:
  `<checkout folder>/.farhelm-spares/<owner>/<name>`. The container name is reserved the way the trash's
  `farhelm-archived-working-copies` is: the occupancy scan reports it occupied, and repository discovery skips it. A
  separate TODO entry will later move the trash itself into a similarly named hidden folder, so implement the
  reservation as a small set of reserved names (or one helper) that a second hidden name can join without another
  special case, rather than a second copy of the archive's one-off check.
- A spare is a plain clone made with today's exact clone command, then kept as is. No post-clone command runs on it.
- A spare only ever appears in its slot complete: a killed build never leaves a partial spare there, and a build that
  finds the slot occupied when it finishes is discarded.
- The supervisor remembers, for each spare, where it lives (the supervisor does not know a host's checkout folders
  between launches, so eviction and discard need this) and the spare folder's identity (so confined removal can check
  it). Whatever the take needs (the remote's name, the checked-out branch) is read from the spare's own `.git` at take
  time; never assume the remote is called `origin`, since a user's `clone.defaultRemoteName` changes it.

**Taking a spare at launch (in the session terminal, inside the existing clone stage):**

- A launch looks only in its own checkout folder's slot. Claiming the spare is a single no-replace rename of the slot to
  a name private to this launch (named after the session and launch generation). Of concurrent launches exactly one gets
  the spare; the others find the slot empty and clone as today. A take never waits on the cache lock or on a builder. An
  empty slot, or anything unexpected about it, means today's clone, unchanged.
- In the claimed spare it runs, visibly: a fetch from GitHub of only what changed (`--prune`, and `--prune-tags` so
  deleted tags go too), then structural checks: the spare's checked-out branch must still exist upstream, and the
  remote's default branch must not have moved to another branch. Pass `-c remote.<remote>.followRemoteHEAD=always` on
  that fetch unconditionally and compare the remote's `HEAD` with the spare's branch: on Git 2.48 and later this notices
  a renamed default branch, and older Git ignores the unknown key and keeps the clone-time value, which is the "accept
  the rare stale branch" behavior the user chose. Verify against the oldest Git the project supports that the key really
  is ignored before relying on it. A deleted old branch is caught on every version by checking that its remote-tracking
  ref survived `--prune`.
- It then moves `.git` and each top-level entry into the launch's allocated checkout folder with no-replace renames and
  removes the empty claimed folder. **It must not rename the spare onto the checkout path**: the supervisor allocates
  that folder itself with a one-winner `mkdir`, records its device, inode and creation time, the shim refuses to clone
  when that identity changed (`run_checkout_preparation` in `crates/farhelm-supervisor/src/launch.rs`), and Delete and
  the trash trust the recorded identity. Moving the contents keeps every one of those rules intact.
- Only after the move does it fast-forward the branch (`git merge --ff-only`), in the final folder, so smudge filters
  and the `post-merge` hook see the session's real path, the same reason the post-clone command runs on the final
  folder.
- Farhelm prints one line in the terminal saying a checkout prepared earlier was used and is being updated from GitHub,
  so the user can tell which path ran. Wording is yours; log it.
- A structural problem before the move (spare corrupt, branch gone or default branch moved, a rename that crosses
  filesystems) falls back to today's clone into the still-empty folder. The shim never deletes a tree: it leaves the
  claimed folder where it is, under its private name in the reserved container, for the supervisor to remove. A failed
  fetch (network or authentication) **fails the clone stage visibly**, exactly as a failed cache refresh does today; it
  does not fall back, because a fallback clone would hit the same failure and prompt for credentials a second time. A
  failure after the move (the fast-forward included) fails the clone stage as today (partial content retained, restart
  refused by the existing interrupted-preparation rule).
- The post-clone command, Claude trust and the agent then run exactly as today, on the final folder.
- Taking a spare does not touch the cache; the builder refreshes it. So a launch makes one network request and at most
  one credential prompt.

**Building the next spare (background, supervisor-owned):**

- After a managed-checkout create on a host (the supervisor then knows repository, URL and checkout folder), the
  supervisor builds the next spare for that repository, with today's exact cache refresh and clone, into a temporary
  folder inside the container. The build for a create starts only after that launch's clone stage has finished (or the
  launch took the spare), so it never races the visible launch for the cache lock.
- It runs through the user's login shell the way a session launch does (so it sees the same `git`, credential helper and
  URL rewrites as the visible clone), but with no terminal attached and prompts disabled (`GIT_TERMINAL_PROMPT=0`, no
  controlling terminal so ssh cannot ask), under a deadline of about ten minutes, at low CPU priority. Any failure
  discards the building folder, is logged, and leaves no spare; a repository that needs a typed password simply never
  gets one.
- One build at a time per host. A build starts only if the slot is empty when it starts, and a trigger for a repository
  whose build is already pending or running is coalesced into it. No queue beyond that.
- Every repository launched into a managed checkout on a host gets a spare. A spare is discarded with its cache when the
  existing 30-day eviction removes the cache.

**Discarding spares (always the supervisor, never the shim):**

- After a create, before building: with the frozen switch off, the supervisor discards that repository's spare instead
  of building; when the create's checkout folder differs from where the repository's spare lives, it discards the spare
  at the old location before building in the new one. Spares of repositories that are not launched again go with their
  cache's 30-day eviction.
- At supervisor startup, together with cache eviction, it removes leftover building folders and claimed folders. Shims
  survive a supervisor restart, so a claimed folder whose launch is still in its clone stage with a live pane is
  skipped; the claim's name (session and generation) is what lets the sweep tell.
- Removal renames to a trash name first, then removes with the confined recursive delete in
  `crates/farhelm-supervisor/src/checkout_trash.rs` (openat2 with `RESOLVE_NO_XDEV`, identity-checked, no symlink or
  mount crossing). Never `remove_dir_all` inside a user's checkout folder.

**Settings switch:**

- A switch in Settings under managed checkouts, on by default, that keeps a checkout ready for faster launches. Its help
  text says what is kept ready, that it costs one extra checkout's disk space per repository and host, and how the
  result differs from a fresh clone (below). It is frozen into each create the way the Claude trust switch is
  (`claude_trust` in `ResolvedGithubCheckout`, `crates/farhelm-proto/src/github_checkout.rs`, with its
  `skip_serializing_if` encoding and fingerprint rationale; stored and routed in the helm like `set_claude_trust`).
  Turning it off stops new builds, and each repository's spare is discarded at its next create on that host (the 30-day
  eviction covers repositories not launched again). Wording is yours; log it. Settings was restructured into sections
  recently; the row goes in the managed checkouts section.

**Specs and docs:**

- SPEC.md "Managed checkouts", the clone paragraph (around SPEC.md:1002-1013): a spare may be used; how the result
  differs from a fresh clone (reflog shows a clone then a fast-forward, `FETCH_HEAD` and `ORIG_HEAD` exist, unchanged
  files keep older modification times, objects from branches deleted upstream linger until Git's own cleanup, hooks and
  filters ran at build time in another folder and run again only for files the update changed, via `post-merge` rather
  than `post-checkout`); the fallback rules; the discard rules; the switch; the reserved folder.
- SPEC.md "VCS neutrality" (around SPEC.md:2275-2290): an explicit, bounded exception for background spare builds of
  repositories the user already requested checkouts of, with their environment and failure behavior.
- SPEC.md's ownership statement (around SPEC.md:3423, "dedicated ownership of the managed checkouts it creates, their
  checkout roots and the trash"): add the spare container.
- SPEC.md uninstall's retained data (around SPEC.md:458): name spares inside checkout folders alongside repository
  caches.
- SPEC_impl.md repository-cache section (around SPEC_impl.md:2253-2286): commands, the take sequence and its claim, the
  builder, where the supervisor remembers spares, the discards and the sweep.
- `docs/github-checkouts.md` ("Repository caches"), `website/src/content/docs/docs/using/start-a-session.mdx` (managed
  checkout section), and both website uninstall pages (`website/src/content/docs/docs/get-started/uninstall.md` and
  `website/src/content/docs/docs/using/update-and-uninstall.md`).
- Changelog fragments per root `AGENTS.md`; the last PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-20), from the planning page, all the recommended options:**

- A checkout made earlier and then updated is acceptable: "Accept it, document the differences, on by default", chosen
  over off by default, over limiting it to repositories without filters, and over shipping only the cache consolidation.
- Background build environment: "Through your login shell, as a launch does, but with no terminal attached" (prompts
  off, a deadline of about ten minutes, failures logged only; a repository needing a typed password never gets a spare),
  chosen over the supervisor's own environment and over an offline build from the cache.
- Location: "A hidden reserved folder, .farhelm-spares/<owner>/<name>", chosen over a visible one. The user added: "also
  add a near term todo entry to move the archive into a similarly named hidden folder" (the planning PR adds that entry;
  it is not this plan's work).
- Post-clone command: "At launch, in the session terminal, on the final folder, as today", chosen over running it ahead
  of time on the spare. A changed post-clone command is therefore not a discard trigger.
- Settings: "A switch under managed checkouts, on by default", frozen per launch.
- Which repositories: "Every repository launched into a managed checkout on that host", dropped with its cache after 30
  days unused.
- Launch-time fetch failure: "Fail the clone stage visibly, as a failed cache refresh does today".
- Cache consolidation: "Once 10 packs accumulate" (`gc.autoPackLimit=10`).
- Renamed default branch: "Use the newer Git option where it exists (Git 2.48+); otherwise accept the rare stale branch
  until the next spare".
- Review gate and mode are in the run instructions below.

**Binding repository constraints:**

- SPEC.md "Managed checkouts" (clone, cache, failure and allocation rules), "VCS neutrality", the ownership statement,
  and uninstall's retained-data paragraph, each changed only as the acceptance criteria say.
- The working-copy allocation and identity model in `crates/farhelm-supervisor/src/working_copies.rs` (module docs,
  `allocate`, `same_directory`) and SPEC.md's identity rule (inode plus creation time); the shim's identity check before
  cloning.
- The trash's confined removal (`checkout_trash.rs`) for any recursive delete inside a checkout folder.
- Root `AGENTS.md`, "Code rules", "Changelog fragments", "Tests and validation", and "Sharing the machine with other
  agents" (tests use the run's own directories); `.agents/test-authoring.md` (tests never change their own process's
  environment: inject Git configuration on child commands, as the existing real-git tests do); `website/AGENTS.md`
  before editing website pages.

**Planner proposals (checked by a fresh-context planning review):** the outline below, including how the supervisor
remembers spares, the builder as a Farhelm step, and the five-PR slicing. A planner proposal is not a requirement: a
smaller design that meets the same acceptance criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main (verify them; line numbers drift):

- The clone stage runs in the shim, in the session terminal: `run_checkout_preparation` (`launch.rs` around 1431) and
  `clone_with_repo_cache` (around 1709). Today's commands: `git init --bare -q` for a missing cache; the cache fetch
  `git -c gc.autoDetach=false -c maintenance.autoDetach=false --git-dir <cache> fetch --prune <url> +refs/heads/*:refs/heads/* +refs/tags/*:refs/tags/*`;
  then `git -c core.commitGraph=false clone --reference <cache> --dissociate -- <url> <cwd>`. SPEC_impl.md around
  2253-2277 documents the same. The child command carries the user's login-shell environment (the shim runs after the
  user's `-l -i` shell) and scrubs repository variables.
- Lock order: the working-copy preparation lock, then the per-repository cache flock via `repo_cache::acquire`
  (`crates/farhelm-supervisor/src/repo_cache.rs`), held through dissociation; `PreparationLock::mark_used` stamps the
  lock file's modification time as the cache's last successful checkout, which the startup sweep (`repo_cache::sweep`)
  uses for the 30-day eviction.
- The supervisor does not know a host's checkout folder between launches: the helm resolves it per create and sends it
  in `ResolvedGithubCheckout.root`. "The checkout folder changed" can therefore only be detected at the next create or
  at startup, from what the supervisor remembered about the spare.
- Name reservation: `occupied_related_names_from_entries` in `working_copies.rs` reports `ARCHIVE_DIR_NAME` occupied
  (the comment explains the collision a titled checkout once caused);
  `crates/farhelm-supervisor/src/repository_discovery.rs` skips the archive by name.
- No-replace renames exist in `working_copies.rs` (`renameat2(RENAME_NOREPLACE)`, macOS `renameatx_np(RENAME_EXCL)`).
- Tests: `d1_fake_git_hook_agent_run_in_order_with_exact_argv` pins the exact argv; `fake_git_script` dispatches on the
  first two argv words; the real-git `d2_real_git_*` tests inject a private Git config with `insteadOf` on the child
  command; `d3_*` cover failure states (including a replaced directory failing the identity check); `d4_*` and
  `concurrent_checkouts_serialize_cache_fetch_and_copy` cover concurrency;
  `crates/farhelm/tests/e2e/github_checkouts.rs` runs the whole path through a real helm and supervisor against a
  private bare remote.
- The abandoned cheaper-clones plan exists only in history (`git log --all -- plans/queue/cheaper-cache-clones.md`). It
  failed the "looks exactly like a plain clone" bar, which the user has now explicitly relaxed for this design. Its
  fixture recipe (bare upstream with branches, annotated tags, a non-`main` default branch, a probe filter in
  `.gitattributes`, injected filter and `core.hooksPath` probes) is worth reusing for the take tests.

Planner proposals for the mechanisms the acceptance criteria leave open:

- What the supervisor remembers about a spare fits in a small record next to the cache (for example
  `repo-cache/<owner>/<name>.spare.json`) holding two things: where the spare lives and the spare folder's identity. The
  take reads nothing from it. Add a field only for a consumer that needs it, logged.
- No spare lock: every actor touches the slot with an atomic rename (the builder publishes with a no-replace rename into
  the slot, a take renames the slot to its claim, a discard renames it to a trash name), and of two renames of the same
  source only one can succeed.
- The builder is a Farhelm step exec'd by the user's login shell
  (`<shell> -l -i -c "exec <farhelm> internal build-spare <spec>"`, no terminal; the name is yours), which calls the
  existing `clone_with_repo_cache` with prompts off, output to the log, and the building folder as its target. That
  keeps "today's exact commands" true by construction (init, fetch, clone, the environment scrub, the commit-graph
  workaround, `mark_used`). The cache lock is taken inside that step, after the user's startup files have run, never by
  the supervisor around the shell: an rc file that hangs without a terminal must not hold the cache lock and stall every
  fallback clone of that repository. `crates/farhelm-supervisor/src/bounded_command.rs` is prior art for the deadline
  and output caps.

Proposed shape (planner proposal), as a linear stack:

1. `perf`: consolidate the repository cache sooner. The `gc.autoPackLimit=10` flag, the exact-argv test, one real-git
   check that a cache with more packs than the limit consolidates, SPEC_impl.md and docs, a changelog fragment.
2. `refactor`: spare bookkeeping. The record, the reserved container name in the occupancy scan and discovery, confined
   discard, startup sweep of spares and leftovers (skipping live claims). Nothing builds a spare yet.
3. `perf`: take a ready spare at launch, with its fallbacks, driven by a spare a test places by hand (production builds
   none yet). Changelog fragment `kind: none`, since nothing reaches it.
4. `perf`: build the next spare in the background, gated by a frozen per-create flag the helm does not send yet.
5. `perf`: keep a spare checkout ready. The helm setting and Settings switch, the frozen flag, SPEC.md and SPEC_impl.md,
   the docs and website pages, the user-facing changelog entry (editing PR 3's fragment), and removal of the TODO.md
   entry. This is the only PR that changes behavior for users.

PRs 3 and 4 can swap.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the supervisor's unit tests for `launch.rs`, `repo_cache.rs`,
`working_copies.rs`, `repository_discovery.rs` and `checkout_trash.rs`, and the managed-checkout e2e
(`crates/farhelm/tests/e2e/github_checkouts.rs`) through `python3 scripts/record-test-run.py`;
`python -B scripts/check-test-sleeps.py` for changed Rust tests (builder tests wait on observable state, never sleeps);
the helm's schema-upgrade test for the new setting column; the Settings section's browser coverage plus one case for the
switch; the website checks `website/AGENTS.md` names; `python3 releasing/check-changelog.py format`.

New tests the outline implies: the reserved name refused by allocation and skipped by discovery; the sweep removing a
spare with an evicted cache and keeping it with a fresh one; leftovers removed; discard refusing a changed identity and
never crossing a mount; take with no upstream change, and after new commits, a new tag, a deleted branch and a deleted
tag (refs and working tree match upstream, the allocated folder's identity unchanged); branch deleted upstream falls
back to a normal clone into the still-empty folder and leaves the claim for the supervisor; fetch failure fails the
stage; two concurrent takes, exactly one wins; interruption after the claim leaves the stage refusing restart; filter
and `post-merge` probes run in the final folder and see only changed files (documenting the difference precisely); the
startup sweep skips a claim whose launch is still in its clone stage; a create in a new checkout folder discards the
spare at the old one, and a create with the switch off discards instead of building; the builder places a spare only via
the final rename, a killed builder leaves nothing in the slot, a builder with prompts off fails cleanly against a
fixture needing credentials, a builder for an occupied slot does not start, and a build never starts before its launch's
clone stage ended; the frozen field's encoding; an e2e where the first launch clones normally and leaves a spare and the
second takes it and is followed by a new one.

## Fallbacks

The spare must never make a launch worse than today's clone, except for the deliberate fetch-failure rule above. If a
piece of the design proves unsafe on a platform (macOS `renameatx_np` semantics on case-insensitive APFS, a checkout
folder on a filesystem without the needed rename), fall back to today's clone on that platform or folder and log it; do
not ship a degraded take path.

Examples of departures that need the scope reassessment: changing the working-copy allocation or identity model instead
of moving contents; running the post-clone command on the spare; building spares for repositories nobody launched; a
second network request at launch beyond the one fetch (for example `ls-remote --symref` for the default branch); any
recursive delete that does not go through the confined removal; any recursive delete in the shim; a lock or queue beyond
the coalescing rule above.

Decisions to log: the terminal line's wording, the Settings row's wording and help text, the record's format and
location, the builder step's name and priority mechanism, the deadline, the followRemoteHEAD check on the oldest
supported Git, and every fragment's kind.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-spare-launch-checkout-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user explicitly chose no-workhorse mode for this plan. The user forbids sending any unit of your own decomposition
of this work to a weaker model or a lower effort than your own. The review gate and the scope reassessment review are
not units of that decomposition: they keep the model and effort this file demands for them. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

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
  `plan/spare-launch-checkout/<nn>-<short-name>` (for example `plan/spare-launch-checkout/01-cache-pack-limit`).
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
