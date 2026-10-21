# Helm launch trust handling

## What this was about

Each path that starts a session used to be responsible for remembering to resolve workspace trust. An earlier inherited
launch bug had carried a trust choice beyond its destination. The maintainer chose a structural refactor that makes an
explicit trust policy mandatory before the helm can send a new or replacement launch to a supervisor, while preserving
behavior and each path's order of checks.

Create and Restart with now require that resolved policy at their internal dispatch boundaries. The supervisor-client
API is unchanged. The plan's TODO entry is removed in the final code PR.

## Things you should know

The starting inventory found no existing route with missing or incorrectly bound trust handling, so there is no fix PR
or behavior-change exemption. The converted paths retain these policies:

- Ordinary New, GUI Clone and Replace with resolve trust against their destination. Enabled managed ownership keeps its
  canonical folder through approval and dispatch.
- Fresh managed-checkout launches use their captured configuration. An accepted retry retains the original launch and
  trust choice; recovery still uses the exact accepted fingerprint without sending a replacement launch.
- Agent create, spawn, inherited spawn and clone choose destination trust before the approval card and carry that same
  launch to dispatch. Their deliberately different and repeated wrapper, connection and approval checks remain.
- Restart with uses the existing session's live checkout membership because it cannot change folders. Plain Restart
  sends no replacement launch and makes no new trust choice.
- Plain Replace copies its live source's exact stored launch, including true, false or omitted trust and historical
  arguments. It does not consult today's trust settings or recompile those arguments.

The plan's example constructor list omitted that last stored-choice policy. It has an explicitly named trust-handling
constructor because normalizing plain Replace would violate the product specification. An accepted checkout recovery
does not need a new constructor: it sends no new launch. The type proves a policy was chosen; existing approval and
connection checks still have their original responsibilities.

The suggested three refactor PRs became two compiling units. REST and agent create callers had to change together when
create began requiring the type; splitting them would need a temporary unchecked conversion. The second PR extracts only
Restart's existing final guard, request and cache-publication sequence. No host-removal lock, supervisor validation or
general launch-admission framework was added.

## Open questions and possible follow-ups

None introduced by execution. The existing launcher custom-permission restoration, template-overwrite wording, stale
YOLO-question and model/effort-test findings remain in the review queue. They concern other contracts, so this plan does
not resolve or remove them. Restart with becoming a mode of the launcher also remains separate work.

## The PRs

- [#2520: require workspace trust handling for creates](https://github.com/scode/farhelm/pull/2520/changes) —
  constructors and every create path, including approved agent launches.
- [#2522: require workspace trust handling for restart edits](https://github.com/scode/farhelm/pull/2522/changes) —
  typed Restart dispatch and the plan's TODO removal; stacked on #2520.

## Checks run, reused and skipped

Both PRs passed `cargo fmt --all -- --check` and `cargo clippy -p farhelm-helm --all-targets -- -D warnings`. The final
TODO edit passed `dprint check TODO.md`. `cargo clippy -p farhelm --bins -- -D warnings` passed on #2520's source and is
reused for the final stack: #2522 extracts common private code, covered by its helm lint, without changing
feature-dependent code or public interfaces. The isolated `python -B scripts/check-test-sleeps.py` check inspected 314
delays with none missing a rationale on #2520; #2522 changes no test, fixture or delay. Dprint has no configured Rust
plugin, so it is not a Rust check here.

Runtime checks used `python3 scripts/record-test-run.py --runner nextest --kind development --tmux required`, pinned
nextest 0.9.148, the matching repository-built tmux 3.7c, four slots and zero retries:

- `cargo nextest run -p farhelm-helm --lib -E <trust/create/approval/recovery selection>`: 34 passes, run
  `7ee2de9a-bc6c-4492-9216-7bd9e9682e95`. Covers constructors, REST create, Replace and Restart with, destination and
  canonical-folder checks, approval-time settings changes, stored-launch preservation, replay vetoes and retained
  checkout recovery.
- `cargo nextest run -p farhelm-helm --lib -E 'test(=agent_requests::tests::inherited_spawn_normalizes_trust_before_approval_and_dispatch)'`:
  one pass, run `9ac0ac23-1de9-47d2-a94f-1b03c75ad267`. This separately fills a singular/plural name-selection gap in
  the first command, rather than retrying a failure.
- `cargo nextest run -p farhelm --test e2e -E <authenticated-clone and lost-fresh-checkout-replay selection>`: both
  selected tests and their fixture setup passed, run `756d7d06-079f-49ff-9f29-d6bcb069d797`. Real process arguments and
  stored launches establish the create/agent-clone/plain-Replace result; the other case verifies accepted checkout
  replay across changed settings and a helm restart.
- `cargo nextest run -p farhelm-helm --lib -E <REST and agent Restart selection>`: nine passes on #2522's source, run
  `99ccdb6b-90dc-4600-be22-de2b249372e8`. Covers trust and named resume, plain consent, structured and command edits,
  invalid selections, conflict replies, unavailable approval facts, wrong replies and refresh publication.

The first three runs cover #2520 at `fd064dd9`; their create, clone, constructor and recovery coverage is reused at
#2522 because those implementations are unchanged. The last run covers #2522 at `1cc03676`. Source capture and runner
reports were complete; retained output reached EOF without truncation or runtime `SKIPPED` markers. There were no failed
runtime attempts in this plan.

The full workspace, browser, desktop, installer and provisioning batteries were skipped: this changes helm-internal
types and a Restart helper, while the affected crate, shipped binary configuration and selected real-stack boundaries
have direct coverage. No UI, client API, installer or provisioning behavior changed. The existing Restart-with e2e tests
call the unchanged supervisor client directly; the focused helm route tests exercise this changed boundary. Main
advanced only through unrelated plan-claim bookkeeping, so no rebase or additional runtime check was needed.

## Review gate's outcome

Each PR passed two independent fresh-context reviews on gpt-6-astra at high effort: one general adversarial review and
one dedicated no-functional-change review. All four completed reviews reported no findings and no observable behavior
change. No aligned-change exemption was used. One initial review launch hit model capacity; retrying the same required
model succeeded. Both PRs remain drafts for the landing monitor.
