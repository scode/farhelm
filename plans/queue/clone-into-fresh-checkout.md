# Clone into a fresh checkout: Clone of a session in a GitHub checkout opens on a new `gh:` checkout

Written against main at 84c10b3b on 2026-10-04 and rechecked against main at 04b4ac8c on 2026-10-05. This is a goal file
for an unattended run, executed through the planning system in `plans/AGENTS.md`. You, the executing agent, have none of
the conversation that produced it; everything you need is here or in the repository files it names. SPEC.md and
SPEC_impl.md stay authoritative over anything this file says, except where this file directs a change to them.

## The goal

A session can start in a fresh GitHub checkout: typing `gh:owner/repo` in the launcher clones the repository into a new
directory under the host's working-copy root (SPEC.md, "Fresh GitHub checkouts"). Today, Clone on such a session opens
the launcher on the source session's existing directory, and SPEC.md says so on purpose ("Ordinary Clone and Replace
start from the source session's actual directory; a new checkout requires an explicit repository selection, a saved
repository setup, or a template that sets one").

After this plan, Clone on a session that lives in a GitHub checkout opens the launcher as if the user had typed
`gh:owner/repo` for that repository and picked a name: the destination is a fresh checkout of the same repository, and
the name defaults to the source session's name with `-clone` appended. When that name's directory is already taken, the
launcher takes the first free one of `<name>-clone-2`, `<name>-clone-3`, and so on.

The maintainer's words, from the planning conversation: "for "gh:" style sessions when I use clone I keep wanting it to
actually create another checkout. i think it makes sense because it does whatever makes sense for the kind. let's plan
out a behavior change so that if you hit clone on such a session, it does the equivalent of you having typed "gh:..."
and then picked a name. default to a name that tacks on -clone to whatever the current session name is."

Acceptance criteria:

- Clone (from the session row's menu and from the session header) on a session whose row carries a checkout opens the
  launcher with a fresh-checkout destination for that repository on the clone's host, and the preview shows the path it
  will create, exactly as a typed `gh:` selection would. "Carries a checkout" means the row's `working_copy` is set, or
  failing that its `github_repo` (Decision 1). This includes the session that created the checkout, a session that
  replaced it, and a session opened in that folder.
- While the user has not edited the name field and the destination is that same repository, the name the launcher
  previews and submits is `<source title>-clone`, or `<source title>-clone-N` for the lowest N ≥ 2 whose directory is
  free on the clone's host (Decision 3). The name field shows that name. The checkout path follows the existing naming
  rule (`checkout_basename` in `crates/farhelm-proto/src/github_checkout.rs`: a title starting with the repository name
  and a hyphen does not repeat the prefix, so cloning `bar-3` of repository `bar` gives `bar-3-clone`).
- Once the user edits the name, the existing explicit-name rule applies unchanged: an occupied name is reported as a
  conflict and never suffixed.
- Choosing an existing folder in the launcher gives exactly today's Clone: the source session's own directory (including
  a subdirectory of the checkout) and its copied title. Choosing the repository again brings the `-clone` default back.
- Choosing a different repository in the launcher drops the `-clone` default: the checkout is unnamed and gets the
  lowest free `<repo>-N`, as the existing copied-title rule already does for Clone into a checkout.
- Changing the clone's host re-previews on the new host, and the suffix search starts over there (a name taken on one
  host may be free on another). A host without a working-copy root shows the existing preview error; choosing a folder
  is the way out, as for any `gh:` launch.
- Replace with, plain Replace, and `farhelm agent clone` keep their current behavior (Decision 2).
- No helm, supervisor, or protocol change. The UI already receives the repository on every session row.
- SPEC.md's "Fresh GitHub checkouts" section and its Clone bullet under the client's actions say all of the above,
  including that a session in a subdirectory of a checkout clones to a fresh checkout's top level, and that a lost-race
  re-preview (the launch reports a conflict, the launcher re-previews and the shown path changes) still needs the user
  to click Launch again. The docs website's "Start a session" page
  (`website/src/content/docs/docs/using/start-a-session.mdx`) mentions the Clone behavior where it covers `gh:`,
  following `website/AGENTS.md` and `website/EDITORIAL_RULES.md`. Code docs that promise Clone keeps the existing
  directory are rewritten (outline, Docs).
- A changelog fragment under `releasing/changelog.d/` (kind `changed`) in the same commit as the behavior change.
- The last PR removes the TODO.md entry "Clone a session in a GitHub checkout into a fresh checkout". It leaves the
  separate entry about making `gh:` launches easier to understand, which is not part of this plan.

## Decisions already made

Settled with the maintainer during planning. Do not reverse them.

1. **Which sessions count:** any session in a checkout, not only the one whose `gh:` launch created it. Otherwise Clone
   would quietly keep the folder after a single Replace.
2. **Only Clone changes.** Replace with keeps opening on the source's existing folder (it starts the same work over in
   place). Plain Replace and `farhelm agent clone` are unchanged.
3. **Name clashes auto-pick the first free suffix** (`<name>-clone-2`, `-3`, ...), rather than showing the conflict and
   making the user edit, but only for the untouched default name.
4. **UI-only suffix search (the planning review's Option A).** The launcher re-previews with the next suffix when the
   preview reports the name occupied, instead of a new preview-request flag that asks the supervisor for the first free
   name. A supervisor-side search would split the requested title from the effective one, which the launcher's
   preview-equals-create check, the retry binding, and the supervisor's create-time name recheck all assume are the
   same, and it would need a protocol field reaching every supervisor only after it upgrades.
5. **Review gate:** two fresh-context reviewers per PR, Claude Opus 5.5 at high effort and gpt-6-astra at high effort,
   no review swarm (How to run, Review gate).
6. **No-workhorse mode** (How to run).

## Implementation outline

Planner proposals unless marked as a decision, grounded in main at 84c10b3b and checked by a fresh-context planning
review. Line numbers are approximate.

**Where the repository comes from.** The UI row has `Session.working_copy: Option<WorkingCopyInfo>` (set for every
session that is a member of a checkout, the innermost one when checkouts nest; carries `repo`) and `Session.github_repo`
(set only on the session whose launch created the checkout): `crates/farhelm-ui/src/lib.rs` around 438. The supervisor
fills both (`project_checkout_metadata` in `crates/farhelm-supervisor/src/store.rs`). Prefer `working_copy.repo`, fall
back to `github_repo`.

**Prefill.** Clone and Replace with both build `CreatePrefill` with `prefill_from(session, generation)`
(`crates/farhelm-ui/src/list/create_form.rs` ~1210-1310); Replace with then calls `mark_replace_with`. Add one
`repo: Option<GithubRepo>` field, set by `prefill_from` from the source, and ignore it when `replace_source` is set. Do
not add a Clone-specific builder. Keep `cwd` as the source's directory: it is what choosing a folder restores, so the
existing test `prefill_from_checkout_metadata_keeps_existing_subdirectory` keeps its `cwd` assertion and gains one for
`repo`. The callers are the row menu (`crates/farhelm-ui/src/list/view.rs` ~2193, ~2254) and the header (`view.rs`
~2294, fed from `crates/farhelm-ui/src/session_view.rs`).

**Applying the prefill.** The reseed effect (`create_form.rs` ~2243-2340) always calls `select_existing_directory` with
the prefill's `cwd`. For a Clone prefill with a repository, select the fresh-checkout destination instead, through the
same sequence every other `gh:` path uses (`remembered_destination.set(None)`, `DestinationDraft::github(repo)`, and a
`preview_revision` bump; see the search-result and saved-setup paths ~546 and ~694). The revision bump matters: without
it, a second Clone of the same row produces an identical preview request, nothing refetches, and the launcher waits for
a preview forever.

**The default name.** Do not write `<title>-clone` into the name field as text. Decide it at read time, extending the
existing rule that sends a copied, unedited title as empty for a checkout destination (`copied_title_ignored` and
`submitted_title`, ~1551-1640, read by the preview authority ~2491, the retry binding ~2718, and submit ~3606). For an
unedited copied title the effective name becomes: the copied title for a folder destination; empty for Replace with into
a checkout and for Clone into a different repository (both unchanged); `<title>-clone` or `<title>-clone-N` for Clone
into the source's repository. The name field shows the effective name (today it shows the preview's basename as
placeholder text when the name is empty; for the `-clone` default, show the name itself). Without this change the
existing copied-title rule wins silently: the seeded default goes out empty and the user gets `<repo>-N`. A source with
an empty title gets no `-clone` default and stays unnamed.

**Suffix search (Decision 4).** One suffix counter (none, then 2, 3, ...) that the effective-name rule appends while it
applies, and one effect that bumps it when the live preview fails because the name is occupied. Cap it (about 50 tries)
and then show the conflict as today; the cap only bounds a misbehaving or misclassified peer. Reset the counter on a new
prefill generation and on a host or repository change. Editing the name needs no reset, because editing turns the rule
off. The suffixed name must be the one that is previewed and submitted, not only the one displayed, or the launch is
refused as a stale preview.

Today the UI cannot tell an occupied name from other preview failures: `api::preview_github_checkout`
(`crates/farhelm-ui/src/api.rs` ~3005) turns every non-2xx into a string, and `PreviewState::Failed` holds only a
message. Make that one function return a typed error that says whether the failure was an occupied name. On this
endpoint, a 409 Conflict without the helm's stale-connection precondition header is the supervisor's occupied-name
refusal (`NameError::Occupied`, mapped in the supervisor's preview handler in
`crates/farhelm-supervisor/src/service/core.rs` ~4590-4618). The helm's other 409 ("requires a host with a stable
installation identity") is reachable only in a race, because the UI never previews without a host identity. Never match
on the message text, which a supervisor writes and may change. A "too long" refusal (a long source title plus the
suffix) is not occupied and must stop the search with its own message. Document at the classification that a future
Conflict added to the preview path would feed the suffix search.

A lost race at launch (the supervisor's create-time recheck, "no longer the free name the preview proposed", `core.rs`
~7419-7440) already makes the launcher re-preview; the counter then advances and the shown path changes, and the user
clicks Launch again. Nothing new is needed for that, and nothing may submit automatically.

**Docs.** SPEC.md: the sentence quoted under The goal, the copied-title paragraph in "Fresh GitHub checkouts", and the
Clone bullet in the client's actions list; keep Replace with's wording consistent ("pre-filled the same way clone
pre-fills it" is no longer exactly true for the destination; say what differs). SPEC_impl.md: check for anything that
repeats the old rule. Code docs that state the old rule: `Session.github_repo` in `crates/farhelm-ui/src/lib.rs` ("it
never turns Clone or Replace into a fresh allocation"), the UI's `WorkingCopyInfo` docs in
`crates/farhelm-ui/src/github_checkout.rs` ("Neither this association nor repo provenance changes an ordinary Clone's
destination"), and `SessionInfo.github_repo` in `crates/farhelm-proto/src/lib.rs` ("not a behavior switch"). Rewrite
them for the new rule rather than deleting the context.

**Tests (proposal).** Unit tests in `create_form.rs` next to the existing prefill and copied-title tests
(`a_copied_unedited_title_is_sent_empty_only_for_a_fresh_checkout` and its neighbors): the effective-name rule for each
case above, the suffix counter's reset rules, and the API error classification (409 with and without the precondition
header, other statuses). Playwright: rewrite the Clone step of "borrowers retain the checkout until the final stopped
session is deleted" in `e2e/tests/github-checkouts.spec.ts`, which asserts Clone keeps the origin's directory and sends
no checkout; add one Clone test modelled on "replace with into a fresh checkout of the source's repository gets the next
free name" in the same file: the first Clone previews `-clone`, launching it and cloning the same source again previews
`-clone-2`, and choosing a folder restores the source's directory. Leave the Replace with tests' expectations alone. Run
the Playwright specs you changed on Chromium and WebKit per root `AGENTS.md`.

**Size.** Small to moderate, all in the UI crate plus docs and one Playwright spec file. The suffix search and its error
classification are the only new mechanism.

**Suggested stack.** One PR is fine. If you split it (for example the API error classification first, then the Clone
behavior with the suffix search), make sure no PR ships Clone's `-clone` default without the suffix search, because a
second Clone of the same session would then show a conflict, which Decision 3 rules out. The behavior change and its
changelog fragment are a `feat` with kind `changed`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-clone-into-fresh-checkout-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

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
  `plan/clone-into-fresh-checkout/<nn>-<short-name>`.
- Shape the stack per the outline. Err on the side of bite-sized PRs, without churn: no code added in one PR and deleted
  in a later one. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits. The behavior change is a `feat` and carries a changelog fragment under `releasing/changelog.d/`
  in the same commit, per root `AGENTS.md` (Releases and the changelog). Validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge (`plans/AGENTS.md`).
- Changes to tests or fixtures follow `.agents/test-authoring.md`; run `python -B scripts/check-test-sleeps.py` per root
  `AGENTS.md` when Rust or browser tests change.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, a nextest selection
of the `farhelm-ui` crate's `list::create_form` and `api` tests, `cargo check -p farhelm-ui --features desktop` (the
launcher is shared with the desktop build), the changed Playwright specs on Chromium and WebKit through the recorder,
the website build when its page changes, and `dprint check` on changed files. Say in the report which checks ran and
why.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to TWO
fresh-context reviewers, as the user demands (Decision 5): a Claude Opus 5.5 agent at high effort, and a gpt-6-astra
agent at high effort (shelled out to the other harness if the executing one cannot reach that model natively). No review
swarm. Both get the same prompt, which carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, its own findings
file (in the scratch directory, one per reviewer), and the acceptance criteria: The goal, Decisions already made, and
the outline above. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md`
verbatim. Address what both reviewers find before moving on, and log the DECISION where you decline a finding. Do not
write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a protocol or supervisor change, a new preview
endpoint, a launcher-side cache of occupied names, a second prefill type for Clone; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the request, the decisions above, this outline, the current diff and
the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular how the effective name is computed and displayed, the typed preview error and how
it classifies an occupied name, the suffix cap and reset points, the PR split, and every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. The planner proposals in the outline are yours to change when the code argues otherwise. Not
covered, and a reason to block per `plans/AGENTS.md` (Executing one plan, step 10) after recording the concrete
tradeoff: changing Replace with, Replace or `farhelm agent clone`; a helm, supervisor or protocol change; shipping the
`-clone` default without the suffix search; and a suffix search that can submit a launch without the user clicking
Launch on the path it shows.

## Done criterion

The plan is complete when its linear stack of draft PRs exists, satisfies the acceptance criteria, has passed the review
gate, and has removed the TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report
through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
