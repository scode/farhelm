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

### Landing

Landed as #2520 and #2522, merged bottom-up, with the same content the report describes, rebased onto this round's main
with one TODO.md conflict resolution. It was the second of two plans in this landing round, after launch-attempt-owner.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained only queue bookkeeping and plan files for future work, and launch-attempt-owner
landed just before it in this round. A fresh-context reviewer and I read the two against each other.

- launch-attempt-owner changes only how the supervisor removes a failed launch's leftover files and what a refused
  create's error says about that cleanup. This plan's create and restart routes do not interpret those refusals; they
  pass them to the browser unchanged, so neither plan relies on the other.
- Restart keeps its order of checks under this plan (check the edit, reach the host, resolve trust on that connection,
  check for YOLO, send), and the supervisor's restart cleanup keeps its rules under launch-attempt-owner.
- Plain Replace still copies the source's stored launch, including its trust choice, without consulting current
  settings.

#### Fixes made during the landing

None to the plan's code. Its TODO.md removal conflicted with launch-attempt-owner's, landed just before it; the landing
kept both removals.

#### Possible follow-ups

None.

#### Checks

Together the two plans add about 1,500 lines and remove about 500 in how the supervisor removes a failed launch's files
and how the helm's create and restart routes resolve workspace trust, and both executors ran selected tests only; that
is why this round ran the whole Rust workspace, plus the browser specs that drive workspace trust, Replace, Restart
with, terminal restart and create retries end to end. All of the following ran on both plans stacked together on this
round's main, including the TODO.md conflict resolution:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the isolated test-sleep check, `dprint check` on every changed
  Markdown, JSON and TOML file, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, the
  workspace debug build and the UI's release web build: all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `68a7a44f-3142-4013-aaef-7d0dc5350e0c` (kept): 4153 passed, 1 failed; 40 tests
  marked as ignored did not run, and the output has no runtime `SKIPPED` message. The failure is a UI test,
  `api::save_order::tests::a_hung_save_releases_waiters_at_their_deadline`, which checks that a read queued behind a
  save that never finishes waits at least 30 milliseconds before giving up; it gave up slightly sooner. Neither plan
  changes the UI crate. The same test has failed this way in three earlier landing rounds, twice in the full workspace
  run (runs `e5bf0834` and `d854fd00`) and once in the UI's desktop-feature run (`d8ff6545`), and passes on its own: 50
  repeats on this round's code (run `e1bfd2c4-f6cd-4627-8ab3-028d8dd8ca26`) all passed. I attribute it to that
  pre-existing timing flake. It is not recorded yet; I am opening a separate draft PR that records it in FLAKES.md, adds
  its Deflake entry to TODO.md and its line to `deflake/known-flakes.txt`.
- `npx playwright test` in `e2e/` for `workspace-trust`, `claude-trust`, `restart-with`, `replace`, `terminal-restart`
  and `terminal-create-idempotency` on Chromium and WebKit, three workers each with its own helm and supervisors, no
  retries, run `34c8e607-2f6d-44ab-b53a-7bd8f1593c35`: 170 passed, none failed.
- Skipped: the desktop feature check and tests, the JavaScript harness and the website build (no UI, asset JavaScript or
  docs changed), the rest of the browser suite (the browser-visible behavior both plans could affect is in the specs
  above), the doctests (no documentation example changed), the desktop smoke test, the native Mac app (not runnable
  here) and the installer (unchanged).

The report's checks ran selected helm and lifecycle tests and skipped the full Rust workspace and the browser suite; the
workspace and the browser specs above ran this round. The report's statement that no rebase was needed held when it was
written; at landing the stack was rebased onto launch-attempt-owner, and the checks above cover that result. Nothing
else in the executor's report became untrue at landing.
