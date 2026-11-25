# Private OS-allocated Tailscale SSH socket directories

Written on 2026-11-25. This is a goal file for an unattended run, executed through the planning system in
`plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you need is here
or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over anything this file says, and
where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one `TODO.md` entry (bucket "Plan follow-ups"). Verbatim:

> **Allocate Tailscale socket folders securely across helm restarts.** On a machine with several accounts, Farhelm can
> lose access to a Tailscale host if its shared SSH socket folder is already occupied. Use a short, private temporary
> directory allocated by the OS. A helm restart can leave earlier SSH masters alive, so retain a trusted way to find
> each route's masters when the host is changed or removed, including those opened before an upgrade. A new directory on
> every start would lose that lookup. Keep the existing short socket paths and verify retirement after a restart.

### Acceptance criteria

1. **OS-allocated private socket directory per Tailscale destination (within Unix socket path limits):**
   - Replace the predictable `/tmp/fhs-<16-char-hash>` creation in `crates/farhelm-helm/src/ssh/tailscale.rs` with an
     OS-allocated (`mkdtemp`-style, mode `0700`) private directory directly under `runtime` (`/tmp` in production), e.g.
     via `tempfile::Builder::new().prefix("fhs-").rand_bytes(10)` producing a short path such as `/tmp/fhs-XXXXXXXXXX`
     (19 bytes), staying well inside macOS's 28-byte remaining budget (`MAX_SOCKET_PATH = 103` minus 75 bytes for
     OpenSSH's `{t|q}{16-char socket_scope}%C` + 17-byte master temp suffix) and Linux's 32-byte budget.
   - Persist the allocated directory basename atomically inside `<state_dir>/tailscale/<sha256(destination)>/` (which
     `ssh/tailscale.rs` already manages with mode `0700` and atomic `NamedTempFile::persist`, requiring no SQLite schema
     migration).
   - On subsequent `prepare` calls (including across helm restarts):
     - Read and validate the persisted **basename** strictly: single normal component, `fhs-` prefix, ASCII alphanumeric
       characters only, bounded length (e.g. `<= 14` bytes so `/tmp/<basename>` is always `<= 19` bytes < 28, and no
       `/`, `..`, `%`, `"`, `\`, `${`), and `runtime.join(basename)` passes `require_private_control_directory`
       (existing directory, not a symlink, owned by `geteuid()`, mode `0700`). Leave `MAX_SOCKET_PATH` enforcement in
       `ssh::control_socket_named` where it already lives so tests passing `tempfile::tempdir()` on macOS
       (`/var/folders/...`) continue to work cleanly.
     - If valid and present, reuse it.
     - If missing (e.g. after reboot wiped `/tmp`), or invalid / no longer private, allocate a fresh `mkdtemp` directory
       and persist it atomically.
     - Guard concurrent `prepare` calls for the same destination without per-destination lockfiles or multi-directory
       lists: because `crates/farhelm-helm/src/ownership.rs` already guarantees one serving helm per state directory,
       serialize `control_directory` inside `spawn_blocking` with a lightweight in-process `std::sync::Mutex<()>` (or
       use `hard_link` create-if-absent and remove the loser's newly created empty directory on `AlreadyExists`). That
       avoids `unsafe libc::flock` duplication and avoids lockfile inode-splitting against `forget_keys`'s
       `remove_dir_all`.
2. **Host-bounded SSH master retirement across helm restarts and upgrades (NEVER glob `/tmp`):**
   - **Hard constraint from maintainer:** NEVER scan or glob `/tmp` for `fhs-*` or delete arbitrary directories matching
     a pattern. Retirement is strictly bounded to the specific host destination being retired.
   - `tailscale::shared_sockets(state, destination)` inspects ONLY:
     1. the exact recorded OS-allocated directory from `<state_dir>/tailscale/<sha256(destination)>/` after
        re-validating the persisted basename and checking `require_private_control_directory` on
        `runtime.join(basename)`, AND
     2. best-effort, that exact destination's single deterministic legacy pre-upgrade directory
        `control_directory_path(state, runtime, destination)` (only if it exists and passes
        `require_private_control_directory`; if missing, or if another user pre-created it and it fails ownership/mode
        checks, ignore it silently rather than failing teardown).
   - Inside each validated directory, retire only OpenSSH control sockets matching the existing
     `[t|q]<16-char base64url><40-char hex>` pattern via `ssh -O exit`.
   - After retiring masters, best-effort remove the recorded directory (and legacy directory if owned/empty) with
     `fs::remove_dir` (non-recursive; fails harmlessly if non-empty).
3. **Also retire Tailscale SSH masters on plain SSH host retarget and Remove:**
   - Extend retirement beyond `hosts::set_tailscale` and environment lifecycle so plain SSH host **retarget**
     (`crates/farhelm-helm/src/hosts.rs`) and plain SSH host **Remove** (including agent Remove and Uninstall via
     `delete_registered_host` in `crates/farhelm-helm/src/provisioning/service.rs`) also call
     `release_shared_connections` before `tailscale::forget_keys` when no remaining Tailscale host row uses that
     destination.
   - Consolidate the refcount check, `release_shared_connections`, and `tailscale::forget_keys` in one shared helper on
     `ProvisioningService` (called by `hosts::retarget` and `delete_registered_host`, while keeping
     `hosts::set_tailscale` from double-retiring since it already retires before committing).
4. **Specs and tests:**
   - Update `SPEC_impl.md` where Tailscale SSH control socket directories and retirement are described.
   - Add unit and integration tests covering:
     - a pre-existing foreign/squatted predictable `/tmp/fhs-<hash>` directory does not block
       `TailscaleSshAccess::prepare` (which allocates a fresh OS-chosen directory and succeeds),
     - `/tmp/<basename>` fits within macOS's 103-byte `MAX_SOCKET_PATH` budget via `control_socket_named(..., 103)`,
     - persisted directory reuse across a simulated helm restart (new `SystemTransport` / fresh `prepare` on the same
       `state_dir`),
     - retirement finding and exiting masters from the persisted directory after a helm restart,
     - retirement finding and exiting pre-upgrade masters from the single deterministic legacy path (and ignoring a
       foreign-owned legacy path without failing teardown),
     - recovery when `/tmp` loses the recorded directory (e.g. after reboot),
     - tampered/invalid persisted pointer values (e.g. `../foo`, wrong prefix, overly long name, symlink, wrong
       owner/mode) rejected safely,
     - plain SSH host retarget and Remove retiring Tailscale masters for that destination (and preserving them when
       another Tailscale host still shares the destination — note `hosts_ssh_destination` is `UNIQUE` in `store.rs`, so
       test the shared-destination preservation via the post-commit window or by calling the helper while a Tailscale
       row for that destination exists in `store`).
   - Remove the covered `TODO.md` entry in the final PR of the stack.

## Decisions already made

### Maintainer decisions (binding)

- **OS-allocated temporary directory:** Use an OS-allocated (`mkdtemp`-style) private `0700` directory rather than a
  salted hash.
- **Plain SSH retarget and Remove retirement:** Also retire Tailscale SSH masters on plain SSH host retarget and Remove,
  strictly bounded to known hosts in Farhelm's database.
- **No `/tmp` globbing:** Explicitly required by the maintainer:
  > If this retirement Is bounded by actual sessions or hosts we know about in our database, then go ahead. If this
  > means just looking for some file pattern forever in all future releases and deleting it, then please come back to me
  > with an update so we can talk more about it. I don't want Farhelm to be permanently deleting, effectively, a glob
  > pattern in the user's temp directory or something like that.
- **Review gate:** Single fresh-context agent with the general charter on `Opus 5.5` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- `crates/farhelm-helm/src/ssh.rs`: `MAX_SOCKET_PATH` is 103 bytes on macOS and 107 bytes on Linux; after `{t|q}` (1),
  `socket_scope` (16), `%C` (40), separator (1), and OpenSSH's master temp suffix (17), `control_dir` has at most **28
  bytes** on macOS and **32 bytes** on Linux. Paths must contain no `%`, `"`, `\`, `${`, or non-UTF-8.
- `crates/farhelm-helm/src/ownership.rs` guarantees one serving helm per state directory.
- `AGENTS.md`: Never mutate process environment variables in tests; never touch the live Farhelm install; annotate every
  test sleep with `// sleep-ok:` passing `scripts/check-test-sleeps.py`.

### Planner-chosen implementation shape

- **Store only the validated basename (e.g. `fhs-XXXXXXXXXX`) inside `<state_dir>/tailscale/<sha256(destination)>/`**
  rather than adding a SQLite schema migration. `ssh/tailscale.rs` already owns that `0700` per-destination folder,
  already receives `state: &Path` (not `HelmStore`), and `forget_keys` already cleans it up when no host references the
  destination.
- **Serialize concurrent `control_directory` allocations with an in-process `std::sync::Mutex<()>` (or `hard_link`
  create-if-absent)** rather than a per-destination lockfile or multi-dir list.
- **Keep `control_directory_path` strictly as a read-only legacy lookup** inside `shared_sockets` (checked best-effort
  for the exact destination, never created).
- **Share one refcount-checked helper on `ProvisioningService`** for `release_shared_connections` + `forget_keys` across
  `hosts::retarget` and `delete_registered_host`.

## Outline

### Existing mechanisms reused

- `crates/farhelm-helm/src/ssh/tailscale.rs` (`prepare_with`, `require_private_control_directory`, `shared_sockets`,
  `sockets_in`, `control_directory_path`, `forget_keys`, `<state_dir>/tailscale/<dest-digest>/`).
- `crates/farhelm-helm/src/ssh.rs` (`MAX_SOCKET_PATH`, `control_socket_named`, `ssh_exact_control_exit_args`).
- `crates/farhelm-helm/src/transport.rs` (`exit_tailscale_masters`, `release_shared_connections`).
- `crates/farhelm-helm/src/hosts.rs` (`set_tailscale`, `retarget`) and `crates/farhelm-helm/src/provisioning/service.rs`
  (`delete_registered_host`, `forget_tailscale_keys`).

### PR stack

1. **`plan/tailscale-private-socket-dirs/01-private-tailscale-socket-dirs`** — Allocate per-destination `mkdtemp`-style
   `0700` directories under `runtime` with `fhs-` + 10 random ASCII alphanumeric chars, persist the validated basename
   atomically under `<state_dir>/tailscale/<dest-digest>/` guarded against concurrent prepares, update `shared_sockets`
   to inspect the recorded directory plus the single exact legacy path (best-effort, zero globbing), clean up empty dirs
   on retirement, update `SPEC_impl.md`, add unit tests covering squatting, macOS 103-byte cap, restart
   reuse/retirement, legacy retirement, and pointer validation, and add the changelog fragment.
2. **`plan/tailscale-private-socket-dirs/02-retire-tailscale-masters-on-retarget-remove`** — Consolidate
   refcount-checked `release_shared_connections` + `forget_keys` on `ProvisioningService` for plain SSH host retarget
   and Remove (avoiding double-retire in `set_tailscale`), add tests covering retarget/Remove and shared-destination
   preservation, amend PR 1's changelog fragment if needed, and remove the covered `TODO.md` entry.

## Fallbacks

- If `hosts.rs` / `provisioning/service.rs` changes make a single PR cleaner than two without exceeding a crisp
  reviewable size, combining into one PR is acceptable; log as a `DECISION`.
- Scope-reassessment triggers (examples, not a blacklist): adding a SQLite schema migration, introducing any `/tmp`
  directory scanning/globbing, or changing ordinary non-Tailscale SSH `ControlPath` behavior.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tailscale-private-socket-dirs-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-tailscale-private-socket-dirs-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation":

- Targeted Rust tests (through `record-test-run.py` with pinned nextest on `PATH`):
  - `cargo nextest run -p farhelm-helm --lib -E 'test(tailscale) | test(/^ssh::/) | test(/^transport::/) | test(/^hosts::/) | test(remove_host)'`
- Formatting, lints, and checks:
  - `cargo fmt --all -- --check`
  - `cargo clippy -p farhelm-helm --all-targets -- -D warnings`
  - `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`
  - `dprint check` on changed Markdown files
  - `python3 releasing/check-changelog.py format`
  - `python3 scripts/check-agents-md.py` (if headings or instruction files change)

Apply `.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live
Farhelm install on this machine (root `AGENTS.md`, "The live install is off-limits").

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the `TODO.md` entry this plan covers ("Allocate Tailscale socket folders securely
  across helm restarts."), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires. Any deferred items added to `TODO.md`
  go under `## Plan follow-ups`.
- Commit messages, PR titles, and changelog fragments follow root `AGENTS.md`, "Conventional Commits" and "Changelog
  fragments". This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you (`plans/.agents/executing.md`, "Executing one plan", step 8); never edit that section.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `Opus 5.5` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular concurrency synchronization (`Mutex` vs `hard_link`), pointer-file format under
`<state_dir>/tailscale/`, PR splitting, and any review finding you decided not to follow. The maintainer will ask for
these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block. Things the maintainer should know and possible follow-ups go into the report as statements.

### Done criterion

The goal is achieved when every PR in the linear stack exists on GitHub as a draft PR and has passed its review gate,
and the latest `## Decisions` entry, if any, is satisfied. Delivering the report is your closing step per
`plans/.agents/executing.md`, "Executing one plan" (step 11), not part of the goal.

## Release note

No release note needed.
