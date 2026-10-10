# Checkout folder in Settings, created on first use, plus four UI layout fixes

Written against main at 51197d61 on 2026-10-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a SPEC change the user decided on, which this plan makes.

This plan has no dependency on another plan.

## The goal

A screenshot preview of the next release's notes showed four things that look broken, and the user asked for all four to
be fixed in one plan. Fixing the first one properly grew into a small feature: the folder Farhelm clones managed
checkouts into becomes a setting in the app.

1. **Managed checkout with no checkout folder.** When the user picks **managed checkout** as a session's destination in
   the session launcher, Farhelm clones a GitHub repository into a new folder under a "checkout root" folder on the
   session's host. There is no default root, and today it can only be set on the helm's machine with
   `farhelm helm checkout-config set-root <path>`. On a fresh install the launcher shows "no checkout root is
   configured; set one with `farhelm helm checkout-config set-root`" in large, unstyled text with the backticks printed
   literally, and Launch stays unavailable. The user wants:
   - the note styled properly;
   - the root as a proper setting in the app's Settings dialog (the gear beside the sidebar's version number). It is one
     setting for all hosts, not per host;
   - when the user attempts a managed checkout and no folder is in effect for the host, the launcher lets them enter one
     and save it on the spot, with a note that it can be changed later in Settings;
   - a folder that does not exist yet is created the first time a checkout is made on a host.
2. **Approval card.** An agent request card (the card that appears near the top of the main area when an agent asks to
   start, stop or change a session) lays out "from host …" and "from session …" in a grid. A session name such as
   "migrate auth tokens" wraps mid-name, leaving "tokens" alone on a right-aligned line below.
3. **Save as template.** In the session launcher's **save as template** panel, each checkbox for a choice to keep sits
   on its own line above its label ("agent type: codex") instead of beside it.
4. **Settings dialog controls.** The redesigned host settings dialog uses switches, but the app's Settings dialog still
   uses checkboxes: the two host choices (`set up new hosts without asking`, `remove hosts without asking`), the three
   sound choices, and the Mac app's `install updates automatically`. The sounds section's help text is also indented
   differently from its heading and choices.

Acceptance criteria:

- A linear stack of draft PRs, in the order under Outline, each passing its review gate.
- On a host with no checkout root in effect, choosing managed checkout in the launcher shows a folder field with a save
  button and a note pointing to Settings. Saving a valid folder makes repository search and the checkout preview work
  without reopening the launcher, and a launch then creates the folder on the host if it is missing.
- Settings shows the all-hosts checkout folder, lets the user change it or clear it (saving an empty field clears it),
  and shows the helm's validation message for a value it refuses. The post-clone command never appears in the UI or in
  any reply the browser can read.
- A host that has its own folder set with the command line (`--host`) is never offered the inline setup, and keeps its
  folder.
- The no-root note and the other launcher checkout notes render as ordinary helper text; no message the launcher shows
  for checkout setup tells the user to run `farhelm helm checkout-config set-root` any more.
- The approval card keeps a session name on one line at the usual card width, the save-as-template checkboxes sit beside
  their labels, and every on/off choice in the Settings dialog is a switch styled like host settings', with the sounds
  help text aligned.
- SPEC.md and SPEC_impl.md say what now holds, in the same PRs that change it.
- Each `feat`, `fix`, `perf`, `style` or `revert` PR has its changelog fragment. The last code PR removes the TODO.md
  entry this plan covers (Near term, "Fix the four UI problems the release-notes preview showed").
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-10),** after seeing the four problems above: "make a plan to fix these (for the settings,
we should use switches not checkboxes). ignore the docs page part." (The docs website's own screenshots are out of
scope.)

**The user's decisions (2026-10-10):**

- One plan for all of it, built as a stack of PRs.
- Switches: the whole Settings dialog, meaning sounds, the two host choices and the Mac app's automatic updates, with
  the SPEC wording updated to match.
- The checkout root, verbatim: "fix the styling but let's also add a proper setting for this ... and when a managed
  checkout is attempted the user should be given an option to enter the checkout location for that host and save it on
  the spot, with a note saying this can later be found in [settings]". Then, on learning that the stored setting is
  global with command-line per-host overrides: "let's just make it a setting in the global settings dialog not per-host
  ... global is fine, simplify." So both the Settings field and the inline launcher save set the all-hosts root;
  per-host overrides stay command-line only.
- A folder that does not exist: "auto-create on first checkout on a given host".
- The post-clone command stays command-line only. A separate TODO.md entry ("Put the post-clone command in Settings")
  covers adding it later; it is not part of this plan.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC.md "Managed checkouts" currently says "There is no default root and no configuration GUI. The root must already
  exist on the target host". The user's decisions override both sentences; this plan rewrites them (Outline, PRs 5 and
  7). It also says "Preview creates no directory and reserves nothing", which stays true: only the create step creates
  the root. SPEC.md's statement that the root's owner and mode are the user's responsibility, and that Farhelm does not
  check them, stays as it is.
- SPEC.md's Settings dialog paragraph calls the host choices "checkboxes"; PR 3 changes the wording.
- SPEC.md's compatibility rules: a helm and supervisor on different protocol versions refuse to connect. This plan
  should need no protocol change (Outline, PR 7). If you find it does, that is a scope reassessment, not a silent bump.
- Root `AGENTS.md`: Conventional Commits; Finishing work; Releases and the changelog; Harness-specific code; Agent
  scratch space; the rule against tests that modify the test process's own environment variables; Sharing the machine
  with other agents.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the mechanisms and test shapes in the outline below. A fresh-context planning review checked
them against main and trimmed them. It dropped a "will be created" notice in the preview, a forced folder mode for
created roots, and a code-span renderer for messages; none of those was requested.

## Outline

Line numbers drift; find the code by name. Checked against main at 51197d61.

### PR 1, `fix`: approval card keeps a session name on one line

`crates/farhelm-ui/src/approvals.rs` renders the requester as `dl.approval-card-requester`. The grid rules are in
`crates/farhelm-ui/assets/app.css` (`.approval-card-requester`, `.approval-card-row`): an `auto-fit` grid with
`minmax(min(100%, 250px), 1fr)` columns, each row a `max-content` label beside a `minmax(0, 1fr)` value, and
`overflow-wrap: anywhere` on the labels. Find why a short value wraps (a column too narrow for label plus value, or the
row's own grid), and fix it in CSS so the value stays whole at the usual card width and wraps only when it genuinely
cannot fit. Assert it in `e2e/tests/approval-layout.spec.ts`, which already stages approval cards: at a typical
viewport, the session value occupies one line box. Look at a screenshot of the card before and after.

### PR 2, `fix`: save-as-template checkboxes beside their labels

`crates/farhelm-ui/src/list/save_template.rs` renders `label.save-template-choice` holding the input and a `span`.
`.save-template-choice` in `app.css` is `display: flex`, yet the box renders on its own line, so a more specific
launcher or form rule overrides it. Find that rule and scope the fix so other launcher labels are unaffected. Extend the
layout check in `e2e/tests/templates.spec.ts` (the save-as-template tests) so a checkbox and its label share a row. Look
at a screenshot.

### PR 3, `style`: switches in the Settings dialog

`crates/farhelm-ui/src/settings.rs` (the two host choices and the desktop-only automatic updates) and the sounds section
in `crates/farhelm-ui/src/sounds.rs` render native checkboxes inside `label.app-settings-choice`. Host settings already
styles native checkboxes as switches with `.host-settings-switch` (see the comment above that rule in `app.css`: labels,
Space activation and disabled states keep working because they stay native inputs). Reuse that class, or rename it to
something shared if that reads better, and lay each choice out like host settings: label text, then the switch at the
right. Align the sounds section's help text with its heading and switches. `restore_automatic_checkbox` finds its input
by `aria-describedby`, so a class change does not break it, but check it. Update tests that select these inputs by role
or class (`e2e/tests/settings.spec.ts`, `e2e/tests/sounds.spec.ts`, and any desktop-feature tests); behavior must not
change. Update SPEC.md's Settings dialog paragraph so the host choices are switches. Look at a screenshot of the dialog
in the web UI.

### PR 4, `fix`: launcher checkout notes styled as helper text

`crates/farhelm-ui/src/list/create_form.rs` renders the repository-discovery message as
`div.launch-composer-repository-note` in two places, and `app.css` has no rule for that class, so it inherits the large
text. Style it like the launcher's other helper text (`.launch-composer-checkout-explanation` is the neighbour to
match), and check that the checkout preview's failure text (`.launch-composer-checkout-preview .create-session-error`)
also reads as an ordinary error line. CSS only. Do not render backtick spans as code: once PRs 5 and 7 reword the
messages, none that reaches the launcher carries backticks. Look at a screenshot with no root configured.

### PR 5, `feat`: the checkout folder in Settings

- **Helm.** Add one route, `GET`/`PUT /api/checkout-root`, with the body `{ "root": string | null }`. PUT with a string
  calls `set_checkout_root(None, …)`, and PUT with `null` (an empty field) calls `clear_checkout_root(None)`, both in
  `crates/farhelm-helm/src/checkout_config.rs`. They already validate (absolute, `~` or `~/…`; no `~user`; no relative
  paths) and bump the configuration revision. Return a refusal's message so the UI can show it. GET returns only the
  all-hosts root. Never return the post-clone command or any host override's value: the browser is deliberately kept
  away from hook text (see the docs on `GithubPreviewResponse` in `crates/farhelm-proto/src/github_checkout.rs`). Follow
  the shape of `GET`/`PUT /api/preferences` in `crates/farhelm-helm/src/preferences.rs` for routing and authentication,
  but not its storage or its queued, silent client writes: the root lives in `checkout_config` and its refusals must
  reach the user. Publish the change on the invalidation feed right away, or have the saving client refetch, rather than
  waiting up to three seconds for the revision watcher (`watch_revision`).
- **Shared field component.** Build one control for this PR and PR 6: a text input, a save button, an inline outcome
  line showing the helm's refusal, and a callback on success. A prop carries the context note, and saving an empty value
  clears. The host settings dialog's edit-field-plus-outcome pattern (`crates/farhelm-ui/src/hosts/settings_dialog.rs`)
  is the local precedent for an explicitly saved field. The Settings dialog's save-as-you-toggle model only suits
  switches.
- **Settings dialog.** Add a "checkout folder" field using it. Its help says what the folder is for: managed checkouts
  are cloned into it on every host, `~` means each host's own home folder, and a missing folder is created on first use
  (true once PR 7 lands; say it in this PR's help text only if PR 7 is in the same stack, which it is). It also says
  that a host given its own folder from the command line keeps that folder.
- **Messages.** Reword the helm's and the supervisor's checkout-root refusals that point at
  `farhelm helm checkout-config set-root` so they point at Settings instead and carry no backticks. Today there are
  three in `crates/farhelm-helm/src/sessions.rs` and three in `crates/farhelm-supervisor/src/service/core.rs`. A
  supervisor that has not been updated keeps sending its old text, which is acceptable.
- **SPEC.** Amend SPEC.md "Managed checkouts": the all-hosts root is set in the Settings dialog or with the CLI, and
  per-host overrides and the post-clone command stay command-line only. SPEC_impl.md's CLI section stays accurate.
- **Tests.** Helm tests for the route: set, clear, a refused value with its message, and that GET carries no hook text.
  A browser test that saves and clears the folder in Settings.

### PR 6, `feat`: set the checkout folder from the launcher

- **Helm signal.** Add a REST-only boolean such as `needs_checkout_root` to the `github-repositories` response
  (`github_repositories` in `crates/farhelm-helm/src/sessions.rs`). It is true exactly where today's code reports "no
  checkout root is configured", meaning no root is in effect for that host: no all-hosts root and no override. It is a
  browser-edge field, not a helm–supervisor protocol change. Do not infer the condition from PR 5's GET or by matching
  message text: a host with a command-line override has a root even when the all-hosts value is empty.
- **Launcher.** When managed checkout is chosen and the response says a root is needed, the launcher shows the shared
  field from PR 5 in place of the bare note, with a note in the user's terms that this folder is used for managed
  checkouts on every host and can be changed later in Settings. Saving sets the all-hosts root. Repository discovery
  does not re-run on a configuration revision change (`observed_checkout_revision` only feeds the preview's
  `config_revision` in `create_form.rs`), so re-run discovery and the preview explicitly after a successful save. The
  template editor shares the destination control but never launches; leave it as it is.
- **SPEC.** Add the inline setup to SPEC.md "Managed checkouts".
- **Tests.** A browser test (extend `e2e/tests/github-checkout-composer.spec.ts`): with no root configured, the field
  appears, a refused value shows the message, and a saved value makes repository suggestions and the preview appear
  without reopening the launcher. A host with a command-line override does not get the field.

### PR 7, `feat`: create a missing checkout folder on first checkout

This is a supervisor-only change in `crates/farhelm-supervisor/src/service/core.rs`. The supervisor already receives the
unexpanded root and is the only party that touches the host's filesystem, so no protocol change is needed. An older
supervisor keeps refusing a missing root, which is acceptable.

- **`resolve_checkout_root`.** It currently fails when `canonicalize` fails. Allow a root that does not exist: resolve
  the deepest existing ancestor, then append the remaining normal components, refusing `..` among them. That gives
  preview and create the same stable `canonical_root`. A purely textual path would break when an ancestor is a symlink
  (macOS `/tmp`, say): the create's binding check against the preview's root would then refuse the first launch as "root
  changed".
- **Repository discovery** (`github_repo_search`) and **preview** (`github_checkout_preview`). A missing root scans as
  empty: no local clones and no occupied names. Without this, a fresh user who saves a folder that does not exist yet
  sees "repository discovery is unavailable" (`repository_discovery_failure` in the helm), the very prompt this feature
  replaces. Preview still creates nothing.
- **Create** (`validate_destination`). Verify the binding against that same resolved path, then create the root and its
  missing parents (`create_dir_all`, following the umask like the checkout's own `create_dir`), then re-resolve and
  require an exact match, refusing as a conflict otherwise. All of this happens before the root identity capture.
  Creation is idempotent, so replays need nothing special. A create refused after this point leaves an empty root
  behind, which is fine.
- **Docstrings.** Update the ones on these functions that promise nothing is created.
- **SPEC.** Replace "The root must already exist on the target host" with the new rule: a missing root is created at the
  first checkout on that host, and preview still creates nothing.
- **Tests.** Supervisor tests:
  - preview and create with a missing root, including missing parents and a symlinked ancestor;
  - discovery with a missing root;
  - a create whose root changed between preview and create is still refused.

  Plus one end-to-end check that a checkout into a missing root works (`crates/farhelm/tests/e2e/github_checkouts.rs`
  has the fixtures).

### Validation

Follow root `AGENTS.md` "Finishing work" per PR, choosing the smallest set that covers the change:

- `cargo fmt --all -- --check` and clippy (`--all-targets`, plus the `--bins` run when supervisor or helm code changes);
- focused nextest selections through `scripts/record-test-run.py` for the touched helm and supervisor tests (with
  `--tmux required` where they use tmux);
- `cargo check -p farhelm-ui --features desktop` for PR 3, since it touches the desktop-only control;
- the named Playwright specs on Chromium and WebKit through the recorder, after the builds the browser suite needs;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
- `dprint check` on changed Markdown;
- `python3 releasing/check-changelog.py format`.

For each UI PR, take a Playwright screenshot of the changed state and look at it before and after, since the problems
were found by looking. Do not run the full battery without a specific risk that calls for it.

Changelog fragments, written for someone running Farhelm after reading `releasing/EDITORIAL_GUIDANCE.md`:

- PRs 1 and 2: `kind: fixed`.
- PR 3: `kind: changed`.
- PR 4: `kind: fixed`; one fragment, or fold it into PR 5's if they read better as one entry.
- PRs 5 to 7: `kind: added`. A single fragment edited by each PR is fine, per the rule for follow-ups to unreleased
  work. It covers setting the folder in Settings, setting it from the launcher, and the folder being created on first
  use. Say that a remote host must run this release for the folder to be created there.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-checkout-folder-and-ui-fixes-log.md` in the parent directory of the checkout you run in, derived as that
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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/checkout-folder-and-ui-fixes/<nn>-<short-name>`, numbered in the order under Outline (for example
  `plan/checkout-folder-and-ui-fixes/01-approval-card-wrap`).
- One PR per item under Outline, in that order, as one linear stack. Keep them bite-sized and avoid churn: code added in
  one PR and deleted in a later one means the stack should have been shaped differently. Within this run, a PR that
  needs correcting is restructured rather than corrected on top, and that applies to PRs an earlier run built too. You
  may merge PRs 5 and 6 into one if splitting them would mean landing the shared field with no caller; log the DECISION.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the PR's part of the outline.

For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. For PR 5, ask the reviewer to confirm that no reply the browser can read carries the post-clone
command. For PR 7, ask it to check the root resolution against symlinked ancestors and `..`, and that a root changed
between preview and create is still refused.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a protocol change, a new persisted
  setting, a per-host GUI, a code-span renderer, or edits well outside the files the outline names beyond tests and
  bookkeeping (examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered. In particular:

- the CSS cause and fix for PRs 1 and 2;
- whether the switch class is reused or renamed;
- the route and response shapes;
- the inline note's wording;
- how a missing root is resolved;
- whether PRs 5 and 6 were merged;
- each Conventional Commit type and changelog kind;
- whether a browser spec ran;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: a protocol bump, per-host settings in the UI,
exposing the post-clone command, or dropping the inline launcher setup.

If no fallback covers it, record the concrete tradeoff and continue with independent work: the stack's lower PRs do not
depend on PR 7. Block per `plans/AGENTS.md` (Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
