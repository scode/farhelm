# Agent retry by request: a keyed agent create replays on the request it was sent as

Written against main at 270ac334 on 2026-10-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file directs a change to them.

## The goal

An agent creates sessions through the helm with `farhelm agent create`, `farhelm spawn` with launch flags, and
`farhelm agent clone`. Each takes an optional idempotency key, so a retry after a lost reply returns the session the
first attempt made rather than starting a second one. The target host's supervisor enforces that: it reserves the key
(scoped to the asking session) with a fingerprint of the create, replays the outcome to a retry whose fingerprint
matches, and refuses a retry whose fingerprint differs as key reuse. That supervisor reservation, and its lifetime
rules, stay exactly as they are.

Today the fingerprint is built from what the helm resolved the request into: the folder, title, parent and the expanded
launch. So a retry after the user edits a template the request names (or, for clone, after the source session's stored
launch, folder or title changes) resolves differently and is refused as key reuse, even though the agent repeated its
request exactly. To paper over the template case, the launch-kinds plan added a helm-side table (`agent_create_bindings`
in `helm.db`, schema 39) that stores each keyed agent create's first resolution and re-sends it on a retry, bounded by
30 days and 256 rows per asking session, with special-case removal when a create is refused. SPEC.md promises the replay
without that bound, so code and spec disagree.

After this plan, an agent's keyed create (all three verbs) is fingerprinted by the request as the agent sent it, not by
what it resolved to. A retry repeating the same request returns the original session even when a template it names (or a
clone's source) was edited in between, with no time or count limit, as long as the request still resolves (see the
acceptance criteria); the same key with a different request is still refused as key reuse. The helm-side resolution
table and everything that exists only for it are deleted.

The maintainer's words, from the planning conversation, rejecting the bounded table: "I am unconvinced this logic even
needs to exist to begin with. ... If my intent is to create a template that I believe to have some contents, there's no
way for me to do so transactionally today anyway." They then chose comparing the request as sent ("option b, lets go for
it"), over both keeping the table and trusting the key outright, and on clone: "same for clone for consistency". And: "i
still want to make sure we delete all that machinery".

Acceptance criteria:

- A keyed `farhelm agent create` or `farhelm spawn` with launch flags, retried with the same key and the same host,
  template names, flags and spawn placement, returns the first attempt's session (or the refusal the supervisor recorded
  for it) even when a template it names was edited in between. `--confirm-yolo` is not part of the request: a retry that
  adds the confirmation the helm asked for is the same request (today's rule, kept).
- A keyed `farhelm agent clone`, retried with the same key, source session, target host, and `--cwd`/`--title` overrides
  (as given, absent included), returns the first attempt's session even when the source's stored launch, folder or title
  changed in between.
- The same key with a different request (any of the fields above differing) is refused as key reuse, as today.
- A keyed `farhelm agent create` without an explicit `--host` is refused, with a message saying that an idempotency key
  needs `--host` (Decision 6). An explicit `--host` already drops the templates' host fields, so a template edit can no
  longer move a keyed retry to another host. `farhelm spawn` (always the asking session's own host) and
  `farhelm agent clone` (whose `--host` is already required) are unaffected. The agent instructions
  (`crates/farhelm/src/agent_instructions.rs`) and the CLI's help say so. The code that refuses it carries a comment
  explaining why (the maintainer asked for this explicitly): a key's record lives only on the host the first attempt
  reached, so a retry must reach that same host, and a host taken from a template could change between the attempt and
  its retry and start a second session elsewhere. Say there too that a host name reassigned to another machine is the
  remaining, accepted gap (Decision 6).
- SPEC.md records the remaining cross-host gap as a known, accepted deficiency (Decision 6): the record of a key lives
  on the host the first attempt reached, so if the `--host` name comes to mean another machine between the first attempt
  and its retry (host names swapped or reassigned), the retry is a new create on that machine and a second session
  starts. Every check a fresh create gets still applies to it.
- A retry that no longer resolves is refused rather than replayed, and that is accepted: a named template was deleted,
  renamed, or edited into something the CLI refuses; the target host is unconnected; for clone, the source was deleted,
  its host is unreachable, or it is legacy. None of these creates a duplicate. SPEC.md must not promise a replay
  "whatever was edited"; word it as a replay "as long as the request still names something the helm can resolve".
- The key-reuse refusal no longer echoes the key: it currently prints the helm's internal scoped form
  (`agent-<session id>-<hash>`), which the agent never typed. Every caller knows the key it sent. Drop the key from that
  shared message for every caller rather than branching on the caller, and likewise from the other message that echoes
  it (`"cannot tell whether intent key {}'s create ever launched, ..."`). The rest of each message stays.
- Deleted: the `agent_create_bindings` table (a new helm schema version drops it), `HelmStore::agent_create_binding`,
  `bind_agent_create`, `unbind_agent_create`, `AGENT_CREATE_BINDING_CAP`, `AGENT_CREATE_BINDING_TTL_SECS`, the
  `StoredResolution` round trip through the store, `no_supervisor_holds_the_outcome` and the unbind-on-refusal logic in
  `create_for_agent`, and the tests that existed only for them. Nothing of that machinery survives in another form,
  including the table's DDL in the schema ladder (outline, Helm).
- SPEC.md says what is compared (the request as the agent sent it), keeps the unbounded promise, and covers clone.
  SPEC_impl.md's paragraph on the binding table is replaced by a description of the request fingerprint.
  `releasing/changelog.d/agent-cli-launch-flags.md` loses "(for up to 30 days)" and reads correctly for the new rule.
- The last PR removes the TODO.md entry "Settle retry limits for template-based session creation".

## Decisions already made

Settled with the maintainer during planning. Do not reverse them.

1. **Compare the request as sent, not the resolution, for agent creates.** Rejected alternatives: keeping the helm table
   (with the bound written into SPEC.md), and trusting the key outright with no comparison (which would silently return
   the first session to an agent that reused a key for a genuinely different create).
2. **Clone gets the same rule**, for consistency.
3. **Delete all of the binding machinery** (acceptance criteria list it).
4. **The key-reuse refusal stops echoing the key.** The maintainer did not want it to name the existing session.
5. **Scope is agent-originated creates only.** The launcher's own creates (the UI and REST) and
   `farhelm spawn
   --inherit-agent` (answered by the session's own supervisor, never the helm) keep the
   resolved-launch fingerprint and their behavior.
6. **Cross-host retries: accepted deficiency, and `--host` required with a key.** Found by the planning review: without
   the helm's stored resolution, a retry goes to whichever host the request resolves to now, and the host the first
   attempt reached is the only one holding the key, so a retry that resolves to another host starts a second session.
   Keeping a helm-side key-to-host record was rejected (it is the machinery Decision 3 deletes). The maintainer: "let's
   record (a) as a known but accepted deficiency, with --host required when wanting idempotency". It is not a security
   boundary: the duplicate is an ordinary fresh create that passes the YOLO guard and every other check on its own, and
   an agent may already create sessions on any host.
7. **Review gate:** two fresh-context reviewers per PR, Claude Opus 5.5 at high effort and gpt-6-astra at high effort,
   no review swarm (How to run, Review gate).
8. **No-workhorse mode** (How to run).

## Implementation outline

Planner proposals unless marked as a decision. Grounded in main at 270ac334.

**Today.** The helm's agent relay is `crates/farhelm-helm/src/agent_requests.rs`: `create_for_agent` (create and spawn
with launch flags) computes `LaunchEditsRequest::digest`, looks up or writes a binding
(`HelmStore::agent_create_binding` / `bind_agent_create` in `crates/farhelm-helm/src/store.rs`, the
`AGENT_CREATE_BINDINGS_SCHEMA` table), resolves with `resolve_agent_create`, dispatches with `dispatch_agent_create` →
`sessions::do_create_session` (`CreateSpec`, in `crates/farhelm-helm/src/sessions.rs`), and unbinds on certain refusals
(`no_supervisor_holds_the_outcome`). `clone_for_agent` reads the source live and dispatches the same way. Keys are
scoped by `asker_scoped_intent_key`. The supervisor's create handler
(`crates/farhelm-supervisor/src/service/handlers.rs`) builds the `IntentClaim` fingerprint with `create_fingerprint`
(`service/core.rs`, `"session_launch_v1"` tuple of parent, cwd, title and launch; a separate versioned encoding for
fresh GitHub checkouts, which agent creates never carry). The key-reuse refusal is in `service/core.rs`
(`"intent key {} was already used for a different create request; ..."`). The wire request is `CreateSession` in
`crates/farhelm-proto/src/lib.rs`, at `PROTOCOL_VERSION` 40.

**Wire and supervisor (proposal).** Add an optional field to `CreateSession` carrying the agent request's fingerprint (a
digest the helm computes). Honored only from the helm's full-authority connection, and refused from a
session-authenticated one, like the fresh-checkout field: unlike `key_lives_with_session`, which can only narrow a key's
lifetime and is therefore ignored there, this field widens what replays (a session could make a create with a different
folder replay its earlier child). When present, the supervisor's `IntentClaim` fingerprint is a fixed versioned tuple of
the digest alone (for example `("agent_request_v1", digest)`), instead of the resolved launch. The digest already covers
the folder, title, spawn parent and placement; the key's scope covers the asking session; the reservation's location
covers the target host. Bound the field by shape (exactly 64 lowercase hex characters, say) rather than threading it
through the create-field accounting. Bump `PROTOCOL_VERSION` per the repository's protocol rules (the pinned-version
test is renamed at every bump) and log the DECISION. Tombstones (`tombstone_fingerprint`) work on any fingerprint
string; confirm a deleted child's replay still behaves as SPEC.md says for each key scope.

**Helm (proposal).** `create_for_agent` sends `LaunchEditsRequest::digest` (it already excludes `--confirm-yolo`) and
resolves fresh on every attempt; `clone_for_agent` sends a digest of its own request (source session id, target host
name, `cwd` and `title` overrides as given, not `confirm_yolo`). Use distinct, versioned digest inputs per verb so a
create and a clone can never collide under one key. The digest is now stored in permanent supervisor reservations, so
hash a dedicated, versioned digest-input type rather than `serde_json` of protocol types like `TemplateFields` that
change whenever the launcher gains a field (a change there would turn outstanding keys into key-reuse refusals after an
upgrade); document that its serialized shape is frozen, the way `create_fingerprint`'s docs treat `session_launch_v1`.
Delete the binding machinery listed in the acceptance criteria. Schema 39 shipped in a release candidate (v0.22.0-rc.5),
so a 39 → 40 step that drops the table is required. To keep the table's DDL out of the ladder, make the 38 → 39 step a
bare version bump, make 39 → 40 `DROP TABLE IF EXISTS agent_create_bindings`, delete `AGENT_CREATE_BINDINGS_SCHEMA` and
its fresh-database interpolation, and drop the table's line and comment from `checkout_config.rs`'s `rewind_to_v26`;
confirm against the store's frozen-DDL and downgrade-ladder tests.

**Upgrade (proposal, follows precedent).** A reservation written before this change holds a resolved-launch fingerprint,
so a retry spanning the upgrade is refused as key reuse, never duplicated. That is the rule the protocol-39 fingerprint
change already accepted (`create_fingerprint`'s "Earlier encodings are retired" note, SPEC_impl.md "Launch-kinds
reservations"). Build nothing for it; say so in SPEC_impl.md.

**Retries that resolve differently (verified during planning).** Every attempt now resolves afresh. A reservation the
first attempt left pending (never launched) is still safe: the supervisor's relaunch (`validate_retry`) rebuilds from
the stored row, not from the retry's inputs. A keyed spawn re-run after its child was deleted now creates a new child
from the templates as they are now (the session-lifetime reservation is pruned with the child), which SPEC.md's "live as
long as the child session does" already allows; correct SPEC_impl.md's sentence that says it uses the stored resolution.

**Tests (proposal).** Replace the binding tests (`a_keyed_retry_keeps_the_first_resolution_unless_the_helm_refused_it`,
`a_binding_stays_exactly_when_a_supervisor_may_hold_the_outcome`, the store's binding tests) with tests of the new rule
at the narrowest level that proves it: the supervisor honors the request fingerprint only from full authority and
replays or refuses by it; a create retried across a template edit replays; a clone retried across a source change
replays; a reused key with a different flag, template list, host or override is refused; the refusal text carries no
key. Keep it small: prove the digest's sensitivity (each field, absent versus present overrides, `--confirm-yolo`
excluded, create and clone distinct) in one pure helm unit test; one supervisor test for honoring, refusing and
replaying by the field; one relay-level test each for "template edited, retry replays" and "clone source edited, retry
replays". No per-field integration tests. Keep a CLI-level or e2e check where one already covers agent keyed retries
(`crates/farhelm/tests/agent_cli.rs`, `spawn_cli.rs`) rather than adding a new harness.

**Docs.** SPEC.md: the idempotency sentence in the agent CLI section ("An idempotency key is bound to the launch the
first accepted request resolved its templates and flags into...") and the clone description; check the spawn paragraph's
key wording too. SPEC_impl.md: the `agent_create_bindings` paragraph and anything else naming the table or the 30-day
bound. The agent instructions in `crates/farhelm/src/agent_instructions.rs` already tell agents to retry with the same
key and exactly the same flags and templates; leave them unless the change makes them wrong. No website page mentions
the bound today; check again. Several code comments state the old invariant and must be rewritten in the documentation
pass: `validate_retry` ("the fingerprint has already proven the request is the same literal string the first attempt
resolved"), `create_session_admitted` ("computed by the handler from the client's literal string"), the comment before
the `IntentClaim` in `handlers.rs` ("The fingerprint binds the resolved bundle..."), `create_fingerprint`'s docs ("Every
create since protocol 39 fingerprints as..."), the proto's `CreateSession.intent_key` ("The resolved launch joins the
fingerprint") and `AgentVerb::Create.intent_key` ("The helm binds it to the launch its first accepted request resolved
to..."), and `create_for_agent`'s and `LaunchEditsRequest`'s docs.

**Size.** Moderate, net deletion. Most of the work is the wire field with its authority check and the tests.

**Suggested stack** (adjust as the code dictates, without churn): one PR for the wire field and the supervisor's use of
it (with the protocol bump), one PR for the helm switching to it, deleting the binding machinery with its schema step,
and the spec, SPEC_impl and changelog changes, then the TODO removal with the last PR. If the first PR alone would leave
the supervisor honoring a field nothing sends, that is fine; it is not churn.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-agent-retry-by-request-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

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
  `plan/agent-retry-by-request/<nn>-<short-name>`.
- Shape the stack per the outline. Err on the side of bite-sized PRs, without churn: no code added in one PR and deleted
  in a later one. Within this run, a PR that needs correcting is restructured rather than corrected on top.
- Conventional Commits. The user-visible change (a keyed agent retry is no longer refused after a template or source
  edit, with no time limit) is a `fix` and carries a changelog fragment under `releasing/changelog.d/` in the same
  commit, per root `AGENTS.md` (Releases and the changelog), unless editing the existing unreleased
  `agent-cli-launch-flags.md` fragment covers it, in which case log the DECISION. Validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge (`plans/AGENTS.md`).
- Changes to tests or fixtures follow `.agents/test-authoring.md`; run `python -B scripts/check-test-sleeps.py` per root
  `AGENTS.md` when Rust tests change.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for Rust test execution (pinned nextest and tmux per `docs/test-run-evidence.md`). Typical
choices, not a checklist: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, nextest selections of the proto crate, the supervisor's handlers and
core idempotency tests, the helm's `agent_requests`, `store` and `checkout_config` modules, and the `agent_cli` and
`spawn_cli` integration targets, plus `dprint check` on changed files. Say in the report which checks ran and why.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes to TWO
fresh-context reviewers, as the user demands (Decision 7): a Claude Opus 5.5 agent at high effort, and a gpt-6-astra
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

Before implementing a substantial departure from the outline above (a new helm table or cache, a second fingerprint path
for non-agent creates, a compatibility layer for pre-upgrade reservations, a migration of existing reservations; these
are examples, not a blacklist), and whenever the same component has needed repeated corrective review rounds, run a
fresh-context review through galaxy-brain with this charter, supplying the request, the decisions above, this outline,
the current diff and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled
out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the wire field's name and authority rule, the fingerprint encoding and its
version tag, what each verb's digest covers, the protocol and helm schema version handling, which tests were deleted
rather than rewritten and why, and every reviewer finding you declined.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. A material scope expansion, a weakened
guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log
entry is not authorization. The planner proposals in the outline are yours to change when the code argues otherwise. Not
covered, and a reason to block per `plans/AGENTS.md` (Executing one plan, step 10) after recording the concrete
tradeoff: keeping any part of the binding machinery, changing the fingerprint of non-agent creates, or letting a reused
key with a different request replay instead of being refused.

## Done criterion

The plan is complete when its linear stack of draft PRs exists, satisfies the acceptance criteria, has passed the review
gate, and has removed the TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must
also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report
through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
