# Triage outcomes: the 2026-11-24 discards

Written against main at 1396fb526 on 2026-11-24. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

Execute the 50 triage outcomes listed under Outline below, as a short stack of batched PRs. Each outcome is recorded in
`TRIAGE_OUTCOMES.md` under the name of its feedback file in `review_feedback_queue/`, and was decided on 2026-11-24 on
the maintainer's fewest-questions triage page (against main c38b93552), or recorded that day under the clearly-settled
rule. The ledger entry's Assessment and the feedback file hold the finding; its Decision and Completion criteria say
what to do, and they bind except where this file's Decisions retarget them. The planner grouped the outcomes from their
ledger entries and did not recheck each finding against current main: line numbers in the Assessments are from c38b93552
and may have drifted, so confirm each finding in current code before changing anything.

### This plan's area, and the 12 others

This plan's area is queue bookkeeping only: the discarded outcomes, with no code or spec change.

This is one of 13 plans that together execute every outcome pending from the 2026-11-24 triage, split by area of code so
they can run concurrently, as the maintainer asked for the previous batch ("yes we definitely need to batch these in a
much smaller set of plans. please try to separate them by area of code so there is a good chance of them running
concurrently.") and confirmed for this one ("doesn't have to be exactly 10. just a reasonable number"). Any of them may
run at the same time as this one. The others, and the areas this plan leaves to them:

- `triage-spec-text-nov24`: prose only: SPEC.md and SPEC_impl.md text (corrections and the new principles the outcomes
  record), user docs pages, CLI help text, and source comments and docstrings that misdescribe shipped behavior.
- `triage-tooling-nov24`: maintainer tooling: release and brick-test tooling under `releasing/`, the CentOS gate, test
  workers, deflake, the drift runner and real-agent tests, agent-screen capture, GUI goldens and docs shots, and the
  scripts under `scripts/`.
- `triage-custom-agents-nov24`: registered custom agents wherever they appear: the launcher's custom-agent handling,
  recents and search, the custom-agent Settings page, the Templates editor and template replay for custom agents, the
  session menu and quick switcher labels, the CLI's and helm's custom-agent validation and template checks, and the
  supervisor's custom-agent pin-correction tests.
- `triage-launcher-nov24`: the session launcher (Enter and button handling, recents, search, Save as template, the
  filled-from marker, its hook and YOLO questions) and the Templates dialog, outside custom-agent handling.
- `triage-settings-nov24`: the Settings dialog outside the custom-agent page: Save and Close, footers, credentials and
  Tensorlake key forms, notifications, agent trust and hook pages and switches, and Settings tests.
- `triage-sessions-ui-nov24`: the desktop app's session list and sidebar, notices and the notification bell,
  interrupted-upload notices, upgrade highlights, the session toolbar, animation ticker and theme colors.
- `triage-terminal-nov24`: the terminal path: the desktop app's terminal view (selection, paging, scrollback, final
  screen, shortcuts during composition), the grid model and link-disclosure code, the vendored terminal crates, and
  their tests.
- `triage-supervisor-nov24`: the supervisor's service: Stop, Restart, Delete, tabs, teardown and sweeps, recovery and
  re-adoption, scope cleanups and its store, plus the e2e suite and its fixtures.
- `triage-hooks-trust-nov24`: persistent hook setup and its hints and warnings, workspace trust (Claude, Codex,
  Antigravity, Pi and the others) on launches and approval cards, and the trust and hook hint bands above the terminal.
- `triage-helm-nov24`: the helm's provider-neutral code and the Hosts panel: SSH and Tailscale hosts, setup and
  provisioning, automatic and manual updates and their sign-in questions, agent requests and their approvals, credential
  delivery, and the CLI.
- `triage-podman-nov24`: the Podman provider: container builds, recipe images and their proxy settings, probes, claims
  and host identity, leftovers and removal, and the container launcher and Settings surfaces.
- `triage-environments-nov24`: Tensorlake sandboxes and the provider-neutral environment code: waking a host at launch,
  Suspend, environment Delete and its late notices, Trash, sandbox leftovers in Settings, and Replace with's fresh-clone
  warning.

Do not change code in those areas unless an outcome of this plan genuinely requires it. When one does, keep the edit to
what the outcome needs, log the overlap as a DECISION, and name it in the report.

Every outcome here is a discard: the PR removes feedback files and index lines and graduates ledger entries, and changes
no code, test, spec or docs. Acceptance criteria 2, 5 and 6 below do not apply to it.

### What each kind of outcome asks for

The kind is read from the outcome's ledger Decision; the outline tags each outcome with it.

- **Discard**: remove the feedback file and its index line and graduate the ledger entry, with no code or spec change.

### Acceptance criteria

1. Every outcome listed under Outline is executed exactly once, in one PR of this stack, as its ledger entry's Decision
   and Completion criteria say, retargeted only by the Decisions below. Each ends one of four ways: fixed as decided;
   judged no change where its criteria allow it; discarded under the unattended fallback; or deferred through the
   complexity gate.
2. A code fix makes the code match the rule its Assessment quotes, or builds what its Decision picked, with a test where
   practical. A fix that changes behavior SPEC.md or SPEC_impl.md describes updates them in the same PR.
3. Each outcome's Execution field, as moved to `TRIAGE_OUTCOMES_ARCHIVE.md`, names its PR (change ID, bookmark and link)
   and, where its criteria leave a choice or it was discarded or deferred, which way it went and why.
4. Each PR removes exactly its outcomes' feedback files and `review_feedback_queue/INDEX.md` lines, moves exactly their
   entries from `TRIAGE_OUTCOMES.md` to the end of `TRIAGE_OUTCOMES_ARCHIVE.md`, and lists them in its description. On
   the stack's tip none of this plan's outcomes remains in `TRIAGE_OUTCOMES.md` or the queue.
5. Each outcome deferred through the complexity gate has its TODO.md entry, in the `## Plan follow-ups` bucket, in the
   PR that removes it from the queue.
6. No PR exceeds about 300 lines of diff (not counting the ledger, the queue removals, changelog fragments or TODO
   entries), unless one outcome's fix needs it on its own.

## Decisions already made

Settled with the maintainer at triage (each outcome's Decision) and on the 2026-11-24 planning page:

- **Ledger decisions are binding:** the triage-page decisions recorded in each outcome's ledger entry are executed as
  they stand in `TRIAGE_OUTCOMES.md`; this plan does not re-litigate them. The batching and complexity gate below change
  how they are executed, not what was decided.
- **Batching:** execute the outcomes as one PR per coherent group (outcomes touching the same module or of the same
  kind), at most about 300 lines of diff per PR. This deliberately overrides the one-PR-per-outcome rule in
  `review_feedback_queue/.agents/execute-outcomes.md` for these plans. Each PR carries the queue removals and ledger
  graduations for exactly the outcomes it executes and lists them in its description.
- **Complexity gate:** an outcome whose fix turns out not to be straightforward, or to exceed its recorded size gate
  (about 100 lines of diff, not counting the ledger, the queue removal or a changelog fragment), or that turns very
  complicated where its criteria carry the maintainer's own complexity gate, becomes a TODO.md entry in the
  `## Plan follow-ups` bucket describing the work, with enough of the finding's detail (what is wrong, where, the rule
  it breaks, the decided fix, and what made it complicated and why) that a later agent needs nothing else. The PR that
  would have carried the fix adds the entry, removes the feedback file and its index line, and graduates the ledger
  entry with an Execution field saying it was deferred to that entry; the report says so too, under "things you should
  know". The plan keeps going; this is not a block. It replaces the ledger criteria's "over that, execution blocks and
  brings the item back to the maintainer", "stop and push back to the maintainer" and "report that back to the
  maintainer" clauses. Adding these entries is the maintainer's instruction, not an entry on your own initiative.
- **Review gate:** one fresh-context `gpt-6.2-sol` at `high` effort agent per PR, with the general review charter quoted
  under Review gate below. Its findings are inputs, not orders.
- **Release note:** this plan's `## Release note` section is the maintainer's answer for the release's highlights. PRs
  still add changelog fragments when their type requires one (root `AGENTS.md`, "Changelog fragments").
- **Unattended fallback:** decided outcomes that are no longer relevant are discarded rather than brought back to the
  maintainer (maintainer, 2026-11-21, reaffirmed on the 2026-11-24 planning page); the wording is under Unattended
  fallback below.
- **No-workhorse mode:** the plans default (`plans/.agents/planning.md`), stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the recorded triage outcomes (each ledger entry's Decision and Completion
criteria, including the maintainer's comments they quote), the maintainer's notes on execution quoted above, and the
decisions above.

**Binding repository constraints:**

- `review_feedback_queue/AGENTS.md` and `review_feedback_queue/.agents/execute-outcomes.md` (read both before changing
  anything under `review_feedback_queue/` or the ledgers); the batching decision above is the only departure from the
  latter, apart from the unattended fallback.
- The SPEC.md and SPEC_impl.md sections each outcome's Assessment cites.
- `.agents/test-authoring.md` for every test added or changed.
- Root `AGENTS.md`, "Code rules", "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and
  FLAKES.md" (for TODO entries), "Frozen material", and its "Instructions by path" for every path you touch.

**Planner proposals:**

- The grouping of outcomes into PRs under Outline, by kind. You may regroup when a group turns out incoherent (a PR that
  would exceed about 300 lines, or outcomes a reviewer cannot judge together), or to put outcomes that touch the same
  module together, logging why as a DECISION; every outcome stays in exactly one PR of this plan.
- Keep each fix to what its outcome needs; no refactors beyond it.

## Outline

1. `plan/triage-bookkeeping-nov24/01-discards`: the discards. Executes:

   - `trailing-set-tailscale-unparks-actor-on-sync-registry-failure.md` (discard): If saving a host's Tailscale setting
     succeeds but refreshing connections immediately hits a database read error,
   - `trailing-osc8-mismatch-warning-bypassed-by-u1160-hangul-filler.md` (discard): Embedding a zero-width Hangul
     Jungseong Filler (U+1160) in a terminal hyperlink label bypasses the
   - `trailing-printed-url-cut-at-contrasting-hue-opens-shorter-address.md` (discard): A printed URL ends early at
     readable text whose colour differs in hue but not brightness, so a plain click opens a
   - `suspend-mixes-list-and-refresh-observations.md` (discard): Suspend can use
   - `trailing-automatic-update-fresh-dev-build.md` (discard): Automatic Update can
   - `trailing-credential-delivery-holds-disconnected-environment.md` (discard): When an environment disconnects during
     initial credential delivery, reconnect and launch can stall while Farhelm
   - `trailing-link-scanner-misses-lookalike-punctuation.md` (discard):
   - `trailing-link-scanner-ignores-all-lookalike-hosts.md` (discard):
   - `trailing-link-scanner-ignores-at-prefixed-host-with-path.md` (discard): Hyperlink text such as `@paypal.com/login`
     is treated as a handle and shows no mismatch warning, though SPEC.md
   - `trailing-link-scanner-treats-site-before-at-as-email.md` (discard):
   - `trailing-link-scanner-treats-dotted-handle-as-claimless.md` (discard): Hyperlink text `@paypal.com` (a handle
     whose name is a familiar site) shows no mismatch warning; whether SPEC.md's
   - `trailing-link-scanner-reads-slash-prefixed-host-as-path.md` (discard): Hyperlink text such as `/paypal.com/login`
     is read as a file path and shows no mismatch warning.
   - `trailing-link-scanner-short-ipv4-is-not-an-address.md` (discard):
   - `trailing-link-scanner-host-glued-to-cjk-prose.md` (discard): A site name
   - `trailing-link-scanner-backslash-path-counts-as-bare-site.md` (discard): Hyperlink text such as
     `github.com\org\repo` is compared as a bare site name, so it stays quiet for any link to that
   - `trailing-link-scanner-exempts-lookalike-idn-endings.md` (discard):
   - `trailing-link-scanner-spacing-mark-after-slash-hides-host.md` (discard): Hyperlink text `/ःpaypal.com/login` (a
     spacing mark after `/`) reads to the scanner as a path, so it shows no
   - `trailing-link-disclosure-joined-reading-treats-no-address-as-match.md` (discard): A multi-row hyperlink whose
     first reading mismatches loses its warning when joining the rows makes the address
   - `trailing-link-disclosure-gap-splits-address-across-rows.md` (discard): A program can split a familiar address
     across a line wrap (`Sign in at github` / `.com/login`) and avoid the
   - `trailing-link-disclosure-letter-test-counts-spacing-marks.md` (discard): A multi-row hyperlink whose next row
     starts with a spacing mark before a familiar address joins that address onto a
   - `trailing-link-text-invisible-spacing-mark-silences-warning.md` (discard): A spacing mark drawn in the background
     colour between `/` and a familiar address is invisible yet counts as seen, so
   - `ensure-hosts-repair-omits-settings-loss.md` (discard): The repair message omits
   - `antigravity-recovery-republishes-old-trust-status.md` (discard): A
   - `antigravity-restart-can-send-stale-trust-alert.md` (discard): A refresh
   - `manual-update-check-can-be-consumed-by-automatic-skip.md` (discard): A manual update request can disappear when it
     joins a background check whose policy decision already skipped
   - `settings-shows-unreadable-updater-as-on.md` (discard): Settings can still show
   - `trailing-linux-add-writes-frozen-install-before-rechecking-systemd-availability.md` (discard): An approved Linux
     setup can install the binary and unit, then fail on reload, when the user service manager became
   - `trailing-retired-lease-still-admit-local-provisioning-command-before-next.md` (discard): An outgoing credential
     replacement can locally start another provisioning operation during retirement. The remote
   - `trailing-retained-environment-queue-checkout-removal-without-waking-trash.md` (discard): After deleting the only
     session using a checkout in a still-running Farhelm environment, an idle Trash view misses
   - `trailing-proxy-probe-failure-handling-remains-partly-unverified.md` (discard): The inspected paths refuse a failed
     proxy read. Whether every failed read stays distinct from an empty result,
   - `trailing-unrelated-manager-settings-block-container-launch-and-cleanup.md` (discard): A place or custom image can
     be refused even when its proxy and path settings are small and readable. A failed place
   - `trailing-delete-anyway-forget-second-tag-whose-absence-was-never.md` (discard): One Delete anyway warning can let
     Farhelm forget multiple unfinished saves, including a tag whose first absence
   - `trailing-saved-absence-replaces-different-places-or-providers-cleanup-error.md` (discard): After a Delete has
     found a missing capture, ordinary retry shows Delete anyway while a separate cleanup blocker
   - `trailing-new-launch-reservation-make-confirmed-delete-refuse-after-changing.md` (discard): When an approved new
     container launch overlaps Delete anyway for the same recipe, Delete can report that a host uses
   - `trailing-delete-anyway-omits-host-needed-to-locate-late-image.md` (discard): With several container hosts, even a
     warning for exactly one capture gives the user no location for the cleanup it
   - `trailing-scoped-shell-teardown-suppresses-failed-history-saving-hangup.md` (discard): If the scope's hangup
     request fails to start, a first-class shell can be forcibly stopped with no chance to save
   - `trailing-portable-teardown-first-find-pinned-shell-when-it-is.md` (discard): One partial process-table read can
     cause a running unscoped shell to be stopped forcibly without a chance to handle
   - `trailing-attach-admits-retained-gone-main-terminal-listings-report-missing.md` (discard): A request to attach can
     be admitted after the supervisor already knows the main terminal is gone.
   - `trailing-delete-verifies-stale-records-host-identity-after-requesting-takeover.md` (discard): If a retained launch
     record names a socket serving another session whose host already has a client, Delete can close
   - `trailing-sampler-date-retained-final-screen-after-agent-exited.md` (discard): If an agent exits during another
     session's sample processing, its retained work screen can refresh its age and
   - `trailing-pending-restart-scope-cleanups-sequential-starvation.md` (discard): If multiple unconfirmed restart scope
     cleanups accumulate on Linux, wedged earlier entries can consume the 60-second
   - `trailing-trash-deleted-drops-removal-report-on-unordered-race.md` (discard): If an unordered Trash refresh fails
     or runs ahead of a Trash delete, the Trash button can drop the freeing-space
   - `trailing-tailscale-retarget-skips-ssh-route-lock.md` (discard): Editing
   - `trailing-ssh-route-lock-leaks-cache-locks-on-delete-race.md` (discard): Removing an SSH host concurrently with a
     probe, sign-in preflight, or Podman check can re-insert an orphaned entry
   - `trailing-podman-leftovers-run-ssh-without-ssh-route-lock.md` (discard): Listing or deleting leftover containers on
     an unused SSH host races turning off Tailscale mode or removing the host,
   - `trailing-reply-and-outcome-debug-can-expose-ntfy-topic.md` (discard): Although ntfy::Settings omits Debug so
     diagnostics never include the ntfy topic, Reply and operations::Outcome
   - `trailing-custom-agent-validate-model-effort-allows-leading-hyphen.md` (discard): Custom agent model and effort
     values can begin with a hyphen (for example `--resume` or `-v`), which Farhelm passes
   - `trailing-custom-agent-validate-omits-display-name-and-command-length-caps.md` (discard): Registering a custom
     agent accepts arbitrarily long display names and executable paths without a size limit.
   - `trailing-session-launch-validate-new-skips-effort-and-custom-model.md` (discard): Widening effort on stored and
     transmitted launches to accept custom free-text strings allows invalid or arbitrary
   - `trailing-versionskew-incarnation-stays-unchanged-on-different-peer-build.md` (discard): If an automatic update
     fails or is refused on a remote host with an older protocol (for example, because the host's

## Fallbacks

- An outcome whose fix needs a design choice that the spec and its ledger entry do not settle, or much more than its
  size gate, takes the complexity gate under Decisions rather than blocking.
- An outcome whose finding is already fixed on main, or whose code no longer exists, follows the unattended fallback
  below.
- Decisions to log: every regrouping, every overlap with another plan's area, every fix-or-no-change judgment and its
  reason, every use of the complexity gate, every outcome discarded under the unattended fallback, every review finding
  declined with its reason, and any criterion found already met or moot.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-triage-bookkeeping-nov24-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-triage-bookkeeping-nov24-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. A PR that changes only comments, docs or spec prose with no doctest or generation role is
documentation-only: no runtime tests, only the formatters and checks that inspect the changed files (`dprint check`, and
`python3 scripts/check-agents-md.py` when a Markdown heading or file it cites changes).

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
without costing quality (independent outcomes in one PR investigated in parallel, say). Concurrent agents that might
otherwise conflict can work in separate jj workspaces whose results you merge back; you still own the stack and every
PR. Launch a sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you
need is not available as a native sub-agent type (a reviewer, for instance), `$scode-harness-shellout` supplies the
launch mechanics.

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

- One reviewable commit, stable bookmark and draft PR per outline step, in a single linear stack, using the `jjstack`
  skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/triage-bookkeeping-nov24/01-discards`.
- Keep each PR to its group and about 300 lines of diff (not counting the ledger, the queue removals, changelog
  fragments or TODO entries). Avoid churn: code added in one PR and deleted in a later one means the stack should have
  been shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and
  that applies to PRs an earlier run built too.
- Each PR executes its outcomes per `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes",
  except that it carries several outcomes: for each one it removes the feedback file and its
  `review_feedback_queue/INDEX.md` line (or narrows the file, for a partial outcome), and moves the entry from
  `TRIAGE_OUTCOMES.md` to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` with its Execution field replaced by the change ID,
  bookmark and draft PR link, plus which way it went where the criteria leave a choice (fixed; no change, and why;
  discarded under the unattended fallback, and why; or deferred to a named TODO.md entry). Record the change ID and
  bookmark before creating the PR, then add the link and update the PR. Each entry's Execution reads
  ``planned in `plans/queue/triage-bookkeeping-nov24.md` `` until then. The PR description lists the outcomes it
  executes.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): a PR of comment and doc corrections is `docs`, test-only changes are `test`, queue bookkeeping is
  `chore`, and a PR that fixes something users see is `fix`. Add a changelog fragment in the same commit when the type
  calls for one (root `AGENTS.md`, "Changelog fragments"): one fragment may cover a PR's user-visible fixes when they
  read as one entry, otherwise one per fix, and a `kind: none` fragment with its reason when nothing user-facing
  changed. This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides. Every other plan of this batch edits them
  too, so expect such conflicts. A conflict anywhere else (SPEC.md included) is a careful rebase
  (`.agents/careful-rebase.md`).
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched natively or through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot
be reached when a review is due and is still unreachable after a reasonable retry, block the plan per
`plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. Never substitute another
reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria: this file's goal, acceptance criteria and
decisions, and for each outcome the PR executes, its full ledger entry and its feedback file. When a PR changes tests or
their fixtures, include the full text of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md`
requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

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
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file. If current code already does what an outcome asks as a
whole, or the code it targets no longer exists, execute that outcome as a discard (queue removal and ledger bookkeeping,
with the reason in its Execution field), log it as a DECISION, and say so in the report. This departs from
`review_feedback_queue/.agents/execute-outcomes.md`, which returns an invalidated decision to the user: the maintainer
asked on 2026-11-21 that decided outcomes no longer clearly relevant be discarded rather than brought back. An
acceptance criterion that is already true by design (one that only says what must be kept) is not a reason to discard.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished; an outcome
that would need such a decision takes the complexity gate instead whenever that keeps the rest moving.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per outline step in the order the outline gives (or
per a regrouping you logged), together executing every outcome listed under Outline exactly once and meeting the
acceptance criteria above, each having passed its review gate and each carrying its outcomes' queue removals and ledger
moves. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. The report maps every outcome to its PR and says how
each ended (fixed, no change, discarded, or deferred to a TODO entry), and names every overlap with another plan's area.
Never edit `plans/` yourself.

## Release note

No release note needed.
