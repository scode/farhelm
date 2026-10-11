# Agents can start sessions in a new managed checkout

Written against main at ef0ad347 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

The TODO.md entry (Near term), "Let agents start sessions in a new managed checkout": the goal is that this works when
typed to an agent: "$farhelm for each of the TODO items above, start a session in a new managed checkout of acme/web,
named after the item". Today it gets a refusal. `farhelm agent create` needs `--cwd` naming an existing directory and
has no repository option, and a template whose destination is a managed checkout is refused on the CLI; SPEC.md says so
directly ("refused on the CLI, which does not create checkouts"), and the helm's launch code enforces it. Give the agent
CLI a way to ask for a fresh managed checkout of a GitHub repository, as a `gh:` launch in the launcher does, with the
matching SPEC change. (The entry's last step, switching the `$farhelm` example in the README and the docs Overview, is
not this plan's: see Decisions.)

Acceptance criteria:

- `farhelm agent create --host <name> --gh acme/web --title "Fix parser" <launch flags>` (with `--github` as an alias of
  `--gh`) asks for a fresh managed checkout of that GitHub repository on that host and starts the session in it, exactly
  as a `gh:` launch in the launcher would: same repository spellings, same checkout root and post-clone command from the
  helm's configuration, same naming (here directory `web-fix-parser`, title "Fix parser"; without `--title`, the next
  free `web-N`), same refusals for a taken name or an over-long name, with no automatic suffix. `--gh` and `--cwd` are
  mutually exclusive.
- `farhelm agent create --template X`, where X's destination is a managed checkout, creates a fresh checkout the same
  way. Agents may write templates whose destination is a managed checkout (`farhelm agent template create`/`edit` gain
  the same `--gh`/`--github` flag). `farhelm spawn` and `farhelm agent clone` keep refusing a managed-checkout
  destination.
- The request waits for approval exactly as any agent create does (none on a host set to "without asking"), and the
  approval card shows the repository and the exact directory the checkout will be created in. The agent YOLO rule
  applies as for any agent create.
- A keyed retry whose first attempt was approved returns that attempt's session (finishing its interrupted start if
  needed) without a second card, and never clones twice. A keyed retry whose first attempt was never approved gets a
  card as usual. The same key with a different request is refused as key reuse.
- `farhelm agent instructions` tells agents how to ask for a managed checkout and how it is named.
- SPEC.md and SPEC_impl.md describe all of the above, including the principle in Decisions about repository sources.
- README.md and the docs Overview page are not touched.
- A linear stack of draft PRs (one by default, see PR discipline), each passing its review gate, with a changelog
  fragment, the last removing (or narrowing) the TODO.md entry as PR discipline says.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-11):** plan this TODO entry.

**The user's decisions (2026-10-11):**

- CLI shape, in the user's words: "--gh aliased as --github - we may add other sources of repos in the future. in fact
  put that in the spec - we do not assume in the UX that there won't be more than one github-like source of repos for
  managed checkouts". So: the flag is `--gh`, with `--github` as an alias, and SPEC.md gains the principle that the
  product's interfaces never assume GitHub is the only source of repositories for managed checkouts; anything that names
  a repository also names its source. (Offered and not chosen: `--repo`, `--cwd gh:...`, both.)
- Templates: allow a managed-checkout destination both when applying a template to `agent create` and when an agent
  writes a template. `farhelm spawn` and `agent clone` keep refusing. (Offered and not chosen: create only; keep both
  refused.)
- Naming: the same as the launcher (described in Acceptance criteria). (Offered and not chosen: auto-suffix on a taken
  name; a separate directory flag.)
- Approval: the same rule as any agent create, with the card gaining the repository and the checkout directory. (Offered
  and not chosen: always ask, even on "without asking" hosts.)
- Keyed retries: return the approved first attempt's session without a card, and say so in SPEC.md for managed
  checkouts; a retry whose first attempt was never approved gets a card as usual. (Offered and not chosen: a card that
  can change nothing before returning.) This amends SPEC.md's "A retry is a new request and is asked about like one" for
  this case.
- Docs: README.md and the docs Overview page are out of scope. In the user's words: "dont change it, i am literally
  doing that now with another agent". The agent instructions text and any website page that this change would make wrong
  are in scope.
- Scope: `agent create` (and agent template writes) only; `agent clone` and `spawn` unchanged.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:**

- SPEC.md "Agent-spawned sessions" (approval, the agent YOLO rule, keyed retries, host naming) and the managed-checkout
  sections (naming, refusals, clone credentials, the post-clone command coming only from helm configuration). SPEC.md
  "Upgrade compatibility and client scale": no upgrade-breaking change.
- SPEC_impl.md "Agent launches from the CLI" and "Owned checkout admission and lifetime". Agent requests stay
  session-authenticated at the supervisor; only the helm's full-authority connection supplies a managed checkout, as
  today.
- Root `AGENTS.md`: "Harness-specific code" if anything touches per-harness behavior (nothing should); Conventional
  Commits; Finishing work; Releases and the changelog; the docs website rules (`website/AGENTS.md`,
  `website/EDITORIAL_RULES.md`) for any page you change; the rule that the docs Overview page is edited only when the
  maintainer asks; Agent scratch space; Sharing the machine with other agents; no tests that modify the test process's
  environment variables.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the mechanisms in the outline below, checked against main at ef0ad347 and corrected by a
fresh-context planning review. That review confirmed that the supervisor needs no change. It caught that the retry
lookup must come before the directory preview and so before the approval card, which produced the keyed-retry decision
above. It also found:

- the launcher's create function cannot be called as it is from the agent path;
- a separate approval-card PR would be churn;
- the card needs no protocol bump;
- spawn shares the resolve step, so its refusal must be kept rather than deleted.

## Outline

Line numbers drift; find the code by name.

Here's what exists today:

- CLI: `AgentCmd::Create` in `crates/farhelm/src/main.rs`; `LaunchFlags::edits` sets
  `destination: cwd.map(TemplateDestination::Folder)`. `--cwd` is declared per command, not in the shared launch flags.
  `TemplateDestination::Github(String)` already exists in `crates/farhelm-proto/src/launcher.rs`, so the request
  (`AgentVerb::Create { host, templates, edits, intent_key, spawn }`) can already carry a repository; the supervisor's
  `validate_agent_verb` only bounds sizes.
- Helm: `agent_requests.rs` `create_for_agent` → `approve_launch` → `dispatch_agent_create`, which builds a `CreateSpec`
  with `github_checkout: None` ("Agent creates never carry a fresh-checkout payload"). `agent_launch::resolve` (shared
  with spawn, `spawn: true`) refuses a managed-checkout destination ("the CLI does not create checkouts; pass --cwd
  ..."), and agent template writes refuse one in `agent_requests.rs`.
- Launcher path: `POST /api/github-checkout-preview` (`sessions::github_checkout_preview`, which asks the supervisor for
  the exact path and returns a preview bound to host, incarnation and installation), then `POST /api/sessions` with a
  `github_checkout` payload. `create_fresh_session` checks the preview, runs the keyed lookup
  (`reconcile_github_checkout` with a `client_identity`) before anything else, builds the `ResolvedGithubCheckout`
  (`github_checkout_resolution`), and calls `do_create_session`, which the agent path also uses. It takes the browser's
  request, compiles the launch itself, and hardcodes `CreateOrigin::User`.
- Keys: a keyed agent create sends `request_fingerprint`, the digest of `agent_create_v1` (`AgentRequestDigest`), and
  the supervisor refuses "a create cannot carry both a request fingerprint and a managed checkout". Managed-checkout
  keyed creates use `client_identity` and the reconcile lookup instead.
- Approval card: `ApprovalAction::Launch` in `crates/farhelm-proto/src/approvals.rs` carries `cwd: String`; it is a
  helm-to-UI type that ships with the helm, not covered by the supervisor protocol version. Template cards in the UI
  already render a "managed checkout of" row.
- Agent instructions: `crates/farhelm/src/agent_instructions.rs`.

The work:

1. **CLI.** `--gh <repo>` with alias `--github` on `agent create`, conflicting with `--cwd`, setting
   `TemplateDestination::Github`. The same flag on agent template create/edit. Repository spellings are validated by the
   shared parser the launcher uses. Not on `spawn` or `clone`.
2. **Resolve.** In `agent_launch::resolve`, stop refusing a managed-checkout destination for `agent create`, and keep
   the refusal when resolving for a spawn (`spawn: true`).
3. **Agent checkout branch in the helm.** For a resolved managed-checkout destination: with a key, run the keyed lookup
   first, using the `agent_create_v1` digest (its JSON unchanged) as the `client_identity` and sending no
   `request_fingerprint`. A found session (or a recorded refusal) is returned without a card. Otherwise preview the
   directory as the launcher does, show the card with `cwd` set to the previewed directory and a new optional `repo`
   field (`serde(default)`, so an older open UI still shows the real directory), and on approval dispatch through a
   helper extracted from `create_fresh_session` (lookup, checkout resolution, `do_create_session`) that takes the
   origin, called here with `CreateOrigin::Agent` so the agent YOLO rule is applied again at dispatch. Do not call
   `create_fresh_session` as it is. A preview that went stale while the card waited fails at the helm or at supervisor
   admission; with a key that records a refusal, so the message must tell the agent to retry with a new key.
4. **Template writes.** Lift the refusal of a managed-checkout destination in agent template writes.
5. **UI.** Render the card's repository row.
6. **Agent instructions.** Describe `--gh`/`--github`, naming via `--title`, and the refusals.
7. **SPEC.md.** In "Agent-spawned sessions":
   - `agent create` and template writes may name a managed checkout, and spawn and clone may not;
   - the card shows the repository and directory;
   - the keyed-retry rule for managed checkouts (an approved first attempt is returned without a card);
   - one clause noting that a retry whose template flipped between a folder and a checkout is refused as key reuse.

   Replace "refused on the CLI, which does not create checkouts" and the template-write sentence. Add the
   repository-source principle where the managed-checkout behavior is specified. **SPEC_impl.md**, "Agent launches from
   the CLI": no fingerprint beside a checkout, and the agent digest serving as the checkout `client_identity`.
8. **Website.** Update any page this makes wrong (search `website/src/content/docs/` for the CLI refusal and for agent
   create's `--cwd` requirement). Never touch README.md or the docs Overview page
   (`website/src/content/docs/docs/index.mdx`).
9. **Bookkeeping.** Changelog fragment, `kind: added`. Remove the TODO.md entry, or narrow it as PR discipline says.

Tests, each proving one requirement:

- CLI parsing: `--gh` and `--github` set the destination, conflict with `--cwd`, and are absent from spawn and clone.
- Helm unit tests replacing the two refusal tests:
  - create with a checkout template resolves;
  - a spawn with one is still refused;
  - a template write with one succeeds.
- A real-stack Rust e2e test, reusing how `crates/farhelm/tests/e2e/github_checkouts.rs` stands in for GitHub:
  - `agent create --gh` with `--title` creates the named checkout and session;
  - a keyed retry after approval returns the same session with no second card and no second clone;
  - the same key with a different request is refused;
  - a taken name is refused.
- A Playwright test, through the fake agent (`crates/farhelm-fixtures/src/fake_agent.rs`, used by
  `e2e/tests/agent-relay.spec.ts`), checks that the card shows the repository and directory and that approving starts
  the session there. Run it on Chromium and WebKit.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo clippy -p farhelm --bins -- -D warnings`;
- the affected Rust tests through the recorder (the changed unit tests, `agent_cli`, and the e2e modules you touched),
  per `.agents/narrow-tests.md`, rather than the whole workspace;
- `cargo check -p farhelm-ui --features desktop` if the UI changed;
- the changed Playwright spec on Chromium and WebKit through the recorder, after the builds the browser suite needs;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`;
- `dprint check` on changed Markdown, and the website build if a `website/` page changed;
- `python3 releasing/check-changelog.py format`.

Do not run the full battery without a specific risk that calls for it.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-agent-managed-checkout-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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
  `plan/agent-managed-checkout/<nn>-<short-name>` (for example `plan/agent-managed-checkout/01-agent-create-gh`).
- One PR by default: the CLI flag, the helm branch, the approval card, the template-apply change and their tests cannot
  be shown working apart. You may split the template-write change (Work step 4, its SPEC sentence and test) into a
  second PR on top if the first is large; log the choice. Do not land an approval-card change that nothing fills in.
  Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect (`feat`). A `feat`
  PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the
  changelog). Validate with `python3 releasing/check-changelog.py format`.
- The last code PR removes the TODO.md entry this plan covers. If main's README.md or docs Overview page still lack the
  managed-checkout `$farhelm` example at that point, narrow the entry to that remaining step instead of removing it.
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
`AGENTS.md` requires. Ask the reviewer to check:

- that no path lets a session-authenticated request supply a checkout;
- that a keyed retry can never clone twice or skip a card its first attempt never got;
- that the agent YOLO rule still applies at dispatch;
- that spawn and clone still refuse a checkout;
- that an older helm or UI meeting the new request or card degrades cleanly per SPEC.md's upgrade rule.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a supervisor or protocol change, a new
  key scheme, a new REST endpoint for agents, or changes to `agent clone` or `spawn` beyond keeping their refusals
  (examples, not a blacklist);
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

- the shape of the helper extracted from the launcher's create path;
- the order of lookup, preview, card and dispatch, and what each refusal tells the agent;
- the SPEC.md wording for the repository-source principle and the retry rule;
- whether template writes got their own PR;
- which website pages changed and why;
- which Rust and browser tests ran;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples:

- skipping the approval card on a request that never had one approved;
- letting a session-authenticated client supply a checkout;
- an automatic name suffix;
- changing `agent clone` or `spawn`;
- touching README.md or the docs Overview page;
- dropping template support.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes (or narrows, as PR discipline says) the TODO.md entry. Open, not
merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing
entry in its log, and stop the watchdog. Never edit `plans/` yourself.
