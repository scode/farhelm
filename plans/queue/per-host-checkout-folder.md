# Set a host's checkout folder in its settings

Written against main at 9e3ab60b on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a SPEC change the maintainer asked for.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Per-host checkout directory.** Settings → Checkouts has one global **Checkout folder** for every host, but one
> directory rarely suits every machine: an absolute path from one machine may not exist or be writable on another, and
> hosts can differ in where projects belong. Let the user set the checkout directory per host in the UI, with the global
> value as the default. Today the only per-host override is `farhelm helm checkout-config set-root --host <id>`, which
> takes a numeric host ID that nothing shows the user.

What the maintainer added while planning, in their words: "per-host setting in the host settings, the existing setting
in regular settings becomes a default when not overridden for a host (default applied at session launch not at host
add)".

What is true today, from a read of main at 9e3ab60b (line numbers approximate):

- The per-host override already works end to end in the helm. The global value is the single-row `checkout_config`
  table; overrides are `checkout_config_host` rows keyed by the host's registry id, deleted with the host and kept
  across retarget or adoption (`crates/farhelm-helm/src/store.rs` around 570-604). Setters, the field-by-field
  resolution and the path-shape validation (absolute, `~` or `~/...`, stored unexpanded) are in
  `crates/farhelm-helm/src/checkout_config.rs` (`set_checkout_root` around 1255, `clear_checkout_root` around 1277,
  `resolve_snapshot` around 1477, validation around 1509). Any change bumps the one global `revision`.
- Launch already resolves per host at discovery, preview and create (`crates/farhelm-helm/src/sessions.rs`,
  `resolve_checkout_config(Some(claim.host))` around 287, 411 and 2091). Nothing copies the global value at host add, so
  "default applied at session launch, not at host add" already holds; keep it that way.
- The browser route `GET/PUT /api/checkout-root` reads and writes only the global value, and its module docs say
  per-host overrides stay out of browser replies (`checkout_config.rs` around 15 and 958-1000).
- SPEC.md, "Managed checkouts" (around 1347-1358), says per-host overrides "remain command-line only" and that
  "browser-readable configuration replies may contain the all-hosts folder and command, never host overrides". SPEC.md
  around 1405-1409 (launcher setup) and around 2109-2117 (Settings → Checkouts) describe command-line overrides keeping
  their folder. SPEC_impl.md says the same around 928-938, in "Checkout configuration and discovery" (around 5154) and
  around 6803. SPEC.md's Topology section (around 244) lists the host settings dialog's fields.
- The host settings dialog is `crates/farhelm-ui/src/hosts/settings_dialog.rs` (`HostSettingsDialog`), rendered and
  driven by `HostsPanel` in `crates/farhelm-ui/src/hosts.rs`, with sections Connection, Appearance and Permissions; each
  field saves on its own through a per-host route (`/api/hosts/{id}/...`, helm `lib.rs` around 766-824, UI `api.rs`
  around 5091-5431). Settings → Checkouts is `crates/farhelm-ui/src/settings.rs` around 683-699 using
  `CheckoutRootField` (`crates/farhelm-ui/src/checkout_root.rs`), which the launcher's inline setup also uses
  (`crates/farhelm-ui/src/list/create_form.rs` around 4378-4481 and 7260-7270).
- Text that points at the command line or the field's old label and must change: `settings.rs` around 694,
  `create_form.rs` around 7262, the supervisor's "no checkout folder" error naming
  `checkout-config set-root or clear-root --host` (`crates/farhelm-supervisor/src/service/core.rs` around 9725-9732) and
  the unit test that pins its wording (around 37560), `docs/github-checkouts.md`, the website pages
  `start-a-session.mdx` ("Checkout management") and `manage-hosts.mdx` (the dialog's sections, and the Settings label
  around 188; `website/AGENTS.md` governs those), and the e2e selectors that match the "Checkout folder" label.
- The "never host overrides" rule has no recorded trust reason: it arrived in #1828 without one, #1858 noted that
  signed-in clients already hold authority to launch commands on hosts, and the checkout preview reply already shows the
  browser each host's effective folder (`canonical_root`). Exposing a host's own folder therefore crosses no trust
  boundary; the post-clone command override stays hidden anyway.
- An unknown host id falls through to a 500 in today's setters, and a read for one answers 200 with no folder, so the
  new route needs its own existence check (404). The dialog's existing per-host routes are POST-only actions with no
  per-host GET, so a `GET`/`PUT` pair is a new shape beside them; that is fine.

### Acceptance criteria

- The host settings dialog has a **Checkouts** section with a **Checkout folder** field, for every host kind (this
  machine, SSH hosts, Tensorlake hosts). It sits between Appearance and Permissions.
- A host with no override shows the field empty with the default greyed as a placeholder ("Default: ~/work", or a "No
  default set" wording when there is no global value). Saving an empty field, or the **Use default** button shown while
  an override is set, removes the override so the host follows the default again. Saving a value sets the override. Help
  text under the field says that an empty field uses the default from Settings → Checkouts and that a change applies to
  new checkouts while existing ones stay where they are.
- Saving checks the path's shape only, as the existing validation does (absolute, `~`, or `~/...`; `~user` is refused);
  the host checks it at launch as it does today. Saving never contacts the host, so a disconnected or suspended host can
  be configured.
- Settings → Checkouts labels the global field **Default checkout folder**, and its help says it is used by every host
  that has no folder of its own and that a host's own folder is set in its settings. The launcher's inline setup for a
  host with no folder keeps setting the default for all hosts, with wording that no longer says overrides come from the
  command line. The supervisor's "no checkout folder" error points the user at the host's settings and Settings →
  Checkouts (it may still mention the command line second).
- The default applies at launch, never copied at host add: changing the default changes what every host without an
  override uses for its next launch.
- SPEC.md and SPEC_impl.md say per-host checkout folders are set in the host settings dialog (and still from the command
  line), that browser replies may carry a host's own folder, and that the per-host post-clone command override stays
  command-line only and out of browser replies. The Topology list of dialog fields gains the folder.
- The `farhelm agent host edit` command does not gain the folder; it stays alias, icon and color.
- Rust tests cover the new route (set, read, clear, an unknown host answering 404, a bad path, and that the post-clone
  override never appears in its reply), and browser tests cover the dialog field (set, inherit placeholder, Use default,
  empty save clears, invalid path refused) and that the launcher's checkout preview for that host then shows the host's
  folder. Launch resolution itself does not change and is already covered.
- The host list reply does not gain the folder: it also answers agents' `hosts` requests, and the dialog reads the
  folder from the new route.
- A `feat` PR with a changelog fragment, which also removes the TODO.md entry quoted above. Docs screenshots showing the
  host dialog or Settings → Checkouts go stale; say so in the report as a follow-up rather than re-capturing them.

## Decisions already made

Settled with the maintainer on 2026-11-08:

- **The field lives in the host settings dialog**, and the Settings value becomes the default for hosts without their
  own, applied at launch, not at host add (the maintainer's words above).
- **Inherit display:** empty field with the default greyed as a placeholder, with **Use default** to clear an override.
  The alternative was showing the effective folder with an "inherited" badge.
- **Post-clone command:** not exposed; the per-host post-clone override stays command-line only. The alternative was a
  second field in the same section.
- **Every host kind** gets the field, Tensorlake included. The alternative was remote hosts only.
- **Save-time check is shape only.** The alternative was asking a connected host whether the folder works.
- **Launcher inline setup keeps setting the default for all hosts**, reworded. The alternative was offering "this host"
  or "all hosts".
- **Agents may not change it** through `farhelm agent host edit`.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

The SPEC change is the maintainer's request: SPEC.md's "command-line only" and "never host overrides" rules are reversed
for the folder by this plan, not by you on your own.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's words, and the decisions above.

**Binding repository constraints:** SPEC.md "Managed checkouts", the launcher setup and Settings → Checkouts passages,
and the Topology dialog list (all to be updated as above); root `AGENTS.md`, "Tests and validation", "Conventional
Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`; `.agents/validation.md`.

**Planner proposals:** the route shape below, the section's placement between Appearance and Permissions, the help
wording, reusing `CheckoutRootField`, and the single PR.

## Outline

The helm already stores and resolves the override, so this is a route, a dialog section, wording and the specs. No
schema change, no new store, no supervisor change beyond the error text.

One PR, `plan/per-host-checkout-folder/01-host-checkout-folder`, typed `feat`:

- a per-host route in the helm, proposed as `GET/PUT /api/hosts/{id}/checkout-root`, reading `{root, default}` (the
  host's own folder or none, and the global value for the placeholder), setting or clearing the override through the
  existing `set_checkout_root`/`clear_checkout_root`, and answering 404 for an unknown host; the UI API client
  functions; Rust route tests;
- the dialog's Checkouts section, reusing `CheckoutRootField` with a scope (all hosts or one host), a label, a
  placeholder and the Use default button, rather than a new editor threaded through the host panel's edit state;
- the Settings, launcher, supervisor-error and docs wording listed above, SPEC.md and SPEC_impl.md updates, browser
  tests, the changelog fragment, and the TODO.md removal.

If the diff turns out too large to review as one, split the route and its specs into a first PR that leaves SPEC.md
describing only what that PR ships; log the DECISION.

### Validation

The helm's checkout-config and session-launch Rust tests and the new route tests, and the browser specs touching
checkout folders and the host dialog (`e2e/tests/settings.spec.ts`, `e2e/tests/github-checkout-composer.spec.ts`, the
host dialog tests in `e2e/tests/sidebar.spec.ts`, `e2e/tests/hosts-panel.spec.ts`, plus `modal-layout`, `modal-focus`
and `tooltip-coverage` for the new control), on the engines `.agents/validation.md` names, all through
`scripts/record-test-run.py`; `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`; `cargo clippy`
and `cargo fmt --check` for the touched crates; `dprint check` on changed Markdown;
`python3 releasing/check-changelog.py format`.

## Fallbacks

Examples of departures that need the scope reassessment: a schema change, exposing the post-clone override, a live check
against the host, changing how launch resolves the folder, or changing the CLI.

Decisions to log: the route shape, the wording chosen, the PR split, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-per-host-checkout-folder-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/per-host-checkout-folder/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

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

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
