# Settings: named credentials for Tensorlake hosts (GitHub tokens, agent logins, endpoint tokens)

Written against main at 47b0d59a on 2026-11-05. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

Two TODO.md entries (bucket "Near term", subsection "Tensorlake"), verbatim:

> - **Agent subscription logins in Tensorlake sandboxes.** Support subscription logins and credential upload for agents
>   in sandboxes, alongside the existing API endpoint settings.
>
> - **GH_TOKEN in Tensorlake sandboxes.** Inject `GH_TOKEN` into Tensorlake sandboxes like containerized sessions, from
>   a named file on the host.

The maintainer replaced the entries' "named file on the host" with a Credentials section in Settings. In their words:

> it's time we don't make the user stick it in a file though. let's actually create a "Credentials" settings in the GUI.
> the existing tensorlake key should be visible and editable there (in addition to the current place), and let's have
> github tokens there too. it needs to be a list of tokens each with a user picked name. it's editable. so the user can
> update the token for a given repo without changing tensorlake recipes that refer to it by name. the GUI must display a
> warning saying the credential is NOT encrypted on disk (we should also add that same warning to the tensorlake side in
> both places).

> similar to GH_TOKEN (see other question) let's have this be a credential managed in the credentials view in settings.
> same story - named credentials, same warnings.

On when a changed credential takes effect:

> "sessions started after" is the hard requirement, but if that means we atomically replace the credential on disk so
> some existing sessions get it (e.g. on restart or agent re-reads the credential) that's fine. no complexity to
> _prevent_ existing agents picking it up. ... we don't want to have to rely on a completely separate CLAUDE_HOME or
> equivalent (e.g. user might want to have their stuff actually configured manually for other things). that's the part
> that's pushing me to making them per-host for hosts (and only per-session for future tensorlake casees where we start
> supporting dedicated sandbox for each session)

### The agreed design (from mockups the maintainer reviewed)

**Settings → Credentials** is a new sidebar section directly after Tensorlake, with a lock icon. Top to bottom:

1. A one-line intro: "Secrets Farhelm hands to hosts for you. Recipes refer to them by name, so replacing a token here
   never means editing a recipe."
2. A prominent red **Not encrypted on disk** warning: "Farhelm stores these in plain text in this helm's state, on the
   machine the helm runs on. Anyone who can read that folder can read them. Farhelm never shows a saved value again."
   (danger-tinted box with a strong left border, the danger colors already in `app.css`).
3. **Tensorlake API key**: the same stored key as Settings → Tensorlake, with the same field, Test, Clear and Save, and
   help "The same key as in Settings → Tensorlake. Changing it here changes it there." Reuse the existing key component
   and routes; the key stays in its own row.
4. Three named lists, each styled like the recipe list (name in mono, a summary line saying which recipes use it, a
   status line such as "Saved 2 Nov" or "Replaced 3 Nov, 14:20", an Edit button), each with an Add button:
   - **GitHub tokens** ("Add GitHub token"). Help: sessions on a host whose recipe names a token get it as GH_TOKEN, and
     git uses it for github.com over HTTPS.
   - **Agent subscription logins** ("Add agent login"). Claude Code: the token `claude setup-token` prints (help: run it
     on any machine where you can sign in to Claude, paste the token it prints; it starts with `sk-ant-oat01-`, lasts a
     year, needs a Pro, Max, Team or Enterprise plan). Codex: the contents of `~/.codex/auth.json`, pasted into a
     textarea or chosen with a "Choose file…" button (help: on a machine with a browser, run `codex login`, sign in with
     ChatGPT, then paste or choose the file).
   - **API endpoint tokens** ("Add endpoint token"): tokens for an API endpoint a recipe points Claude Code or Codex at.

Editing opens in place, like the recipe editor: "‹ Credentials" back button, a title ("GitHub token: work-github", "New
agent login"), the Name (fixed once created; recipe-name rules: lowercase letters, digits, hyphens), the secret field
("Token saved; paste a new one to replace it"), a line naming the recipes that use it, the warning, and Delete / Cancel
/ Save. Delete is unavailable, with a line saying why, while a recipe uses the credential. The agent-login editor has an
Agent segmented control (Claude Code | Codex) above the name. Like the Tensorlake key and ntfy token forms, unsaved text
is discarded on close; do not import the recipe editor's save/discard/keep-editing close guard.

**Settings → Tensorlake:** the API key's small-print storage sentence becomes the same red warning ("Farhelm stores this
key ... can read it."), and the help gains "You can also change it in Settings → Credentials."

**Recipe editor:** a **GitHub** block with a "GitHub token" picker (None or a named token; help says sessions get it as
GH_TOKEN and git uses it for github.com over HTTPS, so fresh checkouts of private repositories work). The **Agents**
block gives each of Claude Code and Codex a segmented choice **None | API endpoint | Subscription**: API endpoint shows
the base URL field and an endpoint-token picker; Subscription shows a login picker limited to that agent's logins. The
recipe editor holds no secrets of its own, so it needs no warning; its help points to Settings → Credentials.

**Codex login note**, in the Codex login editor, stated plainly and only if verified (see Acceptance criteria): when one
copy of a ChatGPT login renews, other copies of the same login are signed out, so use one login per host; to avoid this,
prefer an inference proxy or load balancer through an API endpoint, and the user is responsible for keeping that use
within the provider's terms of service. Never a hedged "may" or "not verified" notice: if you cannot establish a clear
story, leave the note out and say so in the report. Remove-or-keep: the maintainer said "remove the "may log you out"
notice" and then "you can add the note back in the GUI as long as we state it confidently."

### Delivery to hosts

Credentials are **per host**: Farhelm writes each into the host's usual places, replacing each file atomically. A
changed credential value is pushed right away to connected hosts whose recipe uses it, and reaches other hosts when they
connect, before any session starts there; every session started after a change gets the new value, and running agents
may pick it up too (nothing prevents that). Recipe edits that change which credential a recipe names reach existing
hosts at their next connection or setup, like tunnels and endpoints today (the existing recipe-edit note in the editor
stays true).

Design, refined by a fresh-context planning review against the code (planner proposals; change them only with a logged
DECISION if the code says otherwise):

- **One reconcile function per host**, called at setup (where `configure_agents` in
  `crates/farhelm-helm/src/provisioning/tensorlake_add.rs` runs today), in the connect hook that starts tunnels (the
  actor calls it before `serve` publishes Connected, so awaiting it there orders delivery before sessions, and its
  existing `select!` against Suspend/Remove makes it cancellable), and after a credential value is replaced. It computes
  the desired state from the host's recipe and current credentials and compares it with a per-host record of what was
  last delivered (slot to credential name and revision; slots: claude, codex, github, plus the endpoint base URLs), kept
  as one nullable JSON column on the Tensorlake host binding, updated only after a successful write. Credentials carry a
  revision bumped on replacement. Connect does no SSH work when nothing changed.
- **Transport:** reuse the provisioning backend's file operations over the Tensorlake SSH access (as `configure_agents`
  does: read small file, ensure directories, install bytes through stdin). Store the host's home directory at setup (or
  re-derive it) so delivery can find it. Do not add a supervisor protocol verb for file writes. Serialize setup, connect
  delivery and push for one host behind one per-host lock, since all three read-merge-write `~/.claude/settings.json`.
- **Failure never blocks the connection:** on failure continue, surface the reason on the host the way a tunnel failure
  does (amber dot, reason in details), and retry at the next connect or credential change. No timer, no new retry loop.
- **Save of a replaced credential** waits, bounded by the usual SSH command timeout, for connected hosts and reports
  "Saved; not delivered to <host>: <reason>" for any it could not reach, rather than failing the save.
- **Claude Code login:** `CLAUDE_CODE_OAUTH_TOKEN` in `~/.claude/settings.json` `env`, merged like the endpoint keys.
  Subscription mode removes `ANTHROPIC_BASE_URL` and `ANTHROPIC_AUTH_TOKEN` (otherwise the endpoint token wins);
  endpoint mode removes `CLAUDE_CODE_OAUTH_TOKEN`; None removes all three. Only these keys, only under `env`, inside the
  existing merge functions in `crates/farhelm-helm/src/provisioning/agent_endpoints.rs`.
- **Codex login:** `~/.codex/auth.json` (mode 0600), written only when the stored login's revision differs from what was
  last delivered to that host, because Codex refreshes the file itself. When a recipe stops using a Codex login, leave
  `auth.json` in place (Codex owns it after the first write; checking whether it is still Farhelm's copy would mean
  reading credentials out of agent configuration, which the SPEC amendment forbids); the docs say so. Subscription or
  None removes `model_provider` only if it equals `"farhelm"` and removes `experimental_bearer_token` from the farhelm
  provider table, leaving the table.
- **Endpoint tokens** resolve from the named credential instead of the recipe; otherwise as today. The rule that clears
  a saved endpoint token when the base URL changes is retired deliberately: a stale form now carries only a name.
- **GH_TOKEN:** the helm writes the token to one Farhelm-owned 0600 file per host inside the supervisor's state
  directory; the supervisor reads that file at every agent or terminal-tab spawn and every restart relaunch, if present,
  and adds `GH_TOKEN` to the child's environment before the login shell runs, so the managed checkout's clone, the agent
  and tabs all see it. Generalize the container path's `read_token` in `crates/farhelm-supervisor/src/container.rs`
  (trim, refuse empty or NUL, errors disclose no contents, never cached). Never pass it through `LaunchSpec.env` (hidden
  from the clone, and persisted) and never through tmux `-e` (argv); confirm which terminal backend Tensorlake hosts use
  and, if the tmux path is possible there, do not add the token on that path, and say so. GitHub None deletes the file.
  An older supervisor ignores the file; Tensorlake hosts get the matching supervisor at setup or Update, which the docs
  state.
- **`gh`:** add `gh` to the snapshot build's fixed apt packages (`build_script.rs`), from Ubuntu's own package; fall
  back to GitHub's apt repository only if Ubuntu's does not resolve on the Tensorlake builder image or is too old for
  the setup-git form used. When a GitHub slot is first delivered to a host, run gh's own
  `gh auth setup-git --hostname github.com` (verify that form on the installed version; some need `--force`), never with
  the token on its command line or as a remote `GH_TOKEN=...` prefix; fallback: gh's own helper line in the global git
  config. A host whose image lacks `gh` (reused images, and snapshots built before this change until rebuilt) still gets
  GH_TOKEN, skips setup-git, and its details say private clones need `gh`; setup does not fail. The docs list `gh` as a
  requirement for reused images and snapshots.

### Acceptance criteria

- The Credentials section, the Tensorlake section change and the recipe editor match the agreed design above.
- Credentials are stored in a new helm table (next schema version), values never returned by any API, logged, or put on
  any command line, here or on the host; refusals never echo content. Tokens reuse the existing token check (visible
  ASCII, length cap); a Codex login is checked only as a bounded JSON object, with no schema checks that break on Codex
  updates. Delete-while-referenced and recipe-save-with-missing-or-wrong-kind credential checks run in the same store
  transaction.
- The recipe migration drops endpoint tokens saved in recipes (the maintainer chose dropping over moving them); the
  changelog marks it breaking with "re-enter your endpoint tokens in Settings → Credentials".
- With a credential replaced, a session started afterwards on a connected host using it sees the new value; a host that
  was disconnected or suspended gets it before its next session.
- A real-agent check per `.agents/real-agent-tui.md` before relying on the Claude login placement: a token supplied
  through `settings.json` `env` authenticates an interactive Claude Code session, and a fresh home does not stop at a
  first-run onboarding or login screen with it set. **If it does stop there, block**: making it work would mean Farhelm
  writing more of Claude's own configuration (`~/.claude.json`), which the maintainer has not approved.
- Before writing the Codex login note, verify the refresh-token rotation behavior against the installed Codex or its
  documentation; write the note only with a clear, confident story, otherwise omit it and say so in the report.
- SPEC.md: the Settings sidebar list and a Credentials paragraph; the Tensorlake section and recipe text (pickers,
  endpoint tokens by name, per-agent mode, delivery timing); and the Security amendment the maintainer approved, worded
  in substance: for Tensorlake hosts, Farhelm stores agent logins and tokens the user enters in Settings and writes them
  into that host's configuration; it never reads credentials out of any machine's agent configuration, never sends them
  anywhere but those hosts, never proxies them, never logs them, and never passes them as command-line arguments. This
  replaces the conflict with the current "Agent credentials ... live on the host running the agent, in the agent's own
  standard configuration. The system must not extract, proxy, or repurpose agent OAuth credentials." SPEC_impl.md for
  the store, the reconcile record, the GH_TOKEN file and the supervisor read.
- Docs: `website/src/content/docs/docs/using/tensorlake-sandboxes.md` (an incidental docs edit in an unattended run:
  start no preview server and name the changed page in the report), including that reused images need `gh`, that
  `auth.json` stays on a host after a recipe stops using it, and the Codex sharing story if verified.
- Each behavior PR carries its own SPEC text, docs, tests and changelog fragment (later PRs edit the first PR's fragment
  rather than adding near-duplicates), and removes the TODO entry it completes: the subscription-logins entry with login
  delivery, the GH_TOKEN entry with GitHub delivery. The Near term entry "Credentials for containerized sessions and
  ordinary hosts" stays; the planning PR added it at the maintainer's request.
- Tests that touch the supervisor's GH_TOKEN file read inject the path; no test changes process environment variables.

## Decisions already made

Settled with the maintainer on 2026-11-05 on planning pages with mockups:

- **One plan** for GH_TOKEN, subscription logins and endpoint tokens.
- **Credentials section** after Tensorlake; Tensorlake key editable in both places; **named lists**; **names fixed**;
  delete refused while referenced.
- **Codex login** is the user's `auth.json`, pasted or chosen.
- **SPEC amendment** approved, with "add wording that we never log those credentials or pass them as command line
  arguments".
- **Scope:** Tensorlake hosts only, through recipes; a Near term TODO entry for containerized sessions and ordinary
  hosts (added by the planning PR).
- **Per host**, as described under Delivery; per-session credentials wait for a future sandbox-per-session design.
- **Per agent, endpoint or subscription**, never both.
- **Endpoint tokens become named credentials**; tokens already saved in recipes are dropped, re-entered by the user.
- **GitHub:** GH_TOKEN, `gh` installed in Farhelm-built snapshots (a documented requirement for reused images), and gh's
  own `gh auth setup-git` on each host so private checkouts work.
- **Warning** in Credentials and beside the Tensorlake API key.
- **Codex note** back in the GUI only when stated confidently, with the proxy recommendation and the terms-of-service
  responsibility.
- **Code reviewer:** a single fresh-context gpt-6.1-sol at high effort. No substitutes.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entries, the maintainer's words quoted above, and the decisions.

**Binding repository constraints:** SPEC.md's Settings, Tensorlake, recipe and Security sections;
`crates/terminal-host/SPEC.md` (the child's environment arrives over stdin and may hold credentials; read its "Change
policy" before touching it); root `AGENTS.md`, "Code rules" (per-harness behavior lives where
`crates/farhelm-supervisor/src/agent_kind/mod.rs` says), "Tests and validation", "Changelog fragments", "TODO.md and
FLAKES.md"; `website/AGENTS.md`; `.agents/real-agent-tui.md`; `.agents/test-authoring.md`; `.agents/validation.md`. The
existing secret handling in `crates/farhelm-helm/src/tensorlake/config.rs` (stored unencrypted, never returned) and
`agent_endpoints.rs` (bytes through stdin, errors omit content) is the pattern to follow.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): everything under "Delivery to
hosts" marked as design, the table and column shapes, the exact UI wording beyond what the decisions fix.

## Outline

A linear stack, each PR additive and working on its own:

1. `feat:` Credentials store and API (generic over the three kinds), the Settings → Credentials section with the three
   lists and editors, the Tensorlake key block, the warning in both places, SPEC.md's Settings text, a changelog
   fragment.
2. `feat!:` endpoint tokens by name: the recipe migration dropping saved tokens, the endpoint-token picker, the
   reconcile function and per-host record (setup, connect hook, push on replacement, failure surfacing, per-host lock),
   SPEC and SPEC_impl text for recipes and delivery, the breaking fragment.
3. `feat:` subscription logins: the per-agent None / API endpoint / Subscription mode, Claude and Codex slots in the
   reconcile, the Codex write-once rule, the real-agent check, the Codex note if verified, the SPEC Security amendment,
   docs, fragment edit, and removal of the subscription-logins TODO entry.
4. `feat:` GitHub tokens: `gh` in the snapshot build, the supervisor's GH_TOKEN file read, the GitHub slot and
   setup-git, the recipe picker, docs, fragment edit, and removal of the GH_TOKEN TODO entry.

Split a PR further if review size demands, without churn.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt`, clippy, the helm, proto, UI and
supervisor crates' tests for what changed (store, migration, merge functions, reconcile, the GH_TOKEN read), the browser
specs for Settings and the recipe editor through `python3 scripts/record-test-run.py` on Chromium and WebKit,
`dprint check` on changed Markdown, `python3 releasing/check-changelog.py format`, and
`python -B scripts/check-test-sleeps.py` when tests change. No real Tensorlake sandbox: the maintainer has not
authorized one for this plan.

## Fallbacks

Departures that need the scope reassessment: per-session credential delivery; any new supervisor protocol verb; a
separate agent home (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`); reading anything back from a host's agent configuration;
pushing recipe edits (as opposed to credential values) to live hosts; any encryption at rest.

Block if Claude Code needs further edits to its own configuration to accept the token, or if GH_TOKEN cannot reach the
clone without a command line or `LaunchSpec.env`.

Decisions to log: the table and record shapes, the GH_TOKEN file location, the terminal-backend finding, the `gh`
source, the setup-git form, the Codex note outcome, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-settings-credentials-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

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
available as a native sub-agent type (the code reviewer below, for instance), `$scode-harness-shellout` supplies the
launch mechanics.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/settings-credentials/<nn>-<short-name>`
  (for example `plan/settings-credentials/01-credentials-store-and-settings`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments").
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Code review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, agreed design, delivery,
acceptance criteria, decisions and outline). When a PR changes tests or their fixtures, include the full text of
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
Wording in this file that turns out inaccurate (a line number, a file name, a function name) is corrected to the truth.

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
passed its code review gate, and the PRs remove the two TODO.md entries this plan covers. Open, not merged. If a
`## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Blocked

Blocked on 2026-11-06 (claim 6e398e).

### Claude subscription delivery needs permission for first-run setup

This plan adds named credentials in Settings and delivers them to Tensorlake hosts. The independent work is complete in
five reviewed draft PRs: [credential storage and Settings](https://github.com/scode/farhelm/pull/3074/changes),
[named endpoint tokens](https://github.com/scode/farhelm/pull/3077/changes),
[endpoint replacement delivery](https://github.com/scode/farhelm/pull/3081/changes),
[GitHub authentication for private checkouts and terminals](https://github.com/scode/farhelm/pull/3082/changes), and
[Codex subscription logins that preserve refreshed credentials](https://github.com/scode/farhelm/pull/3086/changes).
Claude subscription selection and delivery are the unfinished part; Claude login values can be saved in the catalog but
cannot yet be selected by recipes.

The agreed Claude design writes the subscription token into its settings and requires a fresh host to reach an
interactive agent without onboarding. Installed Claude Code 2.1.318, started in a fresh home and empty directory with
that configuration, stopped at its theme chooser. The token was explicitly fake: this proves the observed onboarding
stop, not successful authentication or how a valid token behaves. Inspection of the installed program also found a
separate onboarding-completion gate. The plan explicitly requires a block before writing additional Claude
configuration. No onboarding answer or additional metadata write was made.

May Farhelm initialize the smallest non-secret first-run fields in Claude's usual configuration, with the required
fields established by an isolated probe, or should this plan defer Claude subscription delivery and land the five
existing drafts? Initialization would preserve other settings and the normal agent home. Deferral avoids managing more
vendor configuration but leaves the agreed Claude feature unfinished. Valid-token authentication remains a required
check and will need a privately supplied test credential; no existing agent credentials were read.

I recommend authorizing the narrow first-run initialization and its isolated verification, so a new Tensorlake host can
use the selected Claude subscription without manual onboarding. This would authorize only the required first-run
metadata; it would not waive the valid-token acceptance check. The public plan answer should record the scope decision
without including a credential.
