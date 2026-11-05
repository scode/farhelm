## What this was about

Five review findings concerned what Farhelm can forget after an OS crash, power loss or failed storage write. Revoked
permissions could return after their revocation was acknowledged. Separately, if the helm's host registry rolled back
and reused a host number, a newly registered host could complete the earlier host's retained, approved launch with the
same request and retry key.

You chose to protect trust while accepting loss of recent history. This plan strengthens permission revocations and
binds saved retry approvals to the requesting host entry. It also documents three existing limits you accepted, rather
than adding recovery machinery for them.

## Things you should know

- Changes to host command, YOLO, public-link and local-network-link permissions now use the existing stronger database
  durability barrier. Changes or clearing of tunnel consent use it too, so acknowledged revocations get the same
  protection as browser-token rotation. Unrelated preference writes keep their ordinary durability. This reuses the
  existing database helper; there is no schema change or separate store.
- Retry approvals now follow the host entry's random identity, so a number reused after registry rollback carries no
  earlier entry's approval. Requests already in flight across this upgrade ask for approval again. If approved, that
  retry creates a new session or checkout instead of returning the earlier one, as accepted in the plan.
- On macOS, even the stronger database writes are durable only up to the drive's cache; SQLite does not request
  `F_FULLFSYNC`. The specification now states this limit for both the supervisor and helm. You accepted it, judging the
  extra protection marginal and expecting most Macs running Farhelm to have batteries.
- The specification now explicitly accepts forgetting a conversation Farhelm learned in the last moments before an OS
  crash or power loss, like other recent state. Restart may then have no conversation to resume or may resume the
  previously recorded one. This clarifies existing behavior and leaves ordinary supervisor-outage recovery unchanged.
- A full disk now explicitly falls under the accepted unhealthy-filesystem outcomes. An existing Resume offer can be
  lost if Restart with fails and the write restoring that offer also fails. This is a specification clarification; it
  adds no code change or new recovery promise, and keeps the boundary protecting the helm and other hosts.

The two code fixes stayed within the agreed size gates: 100 and 96 changed code, test and specification lines,
respectively, excluding ledger updates, queue removals and changelog fragments. All five outcomes record their execution
and remove their matching feedback items.

## Open questions and possible follow-ups

None. No product decision remains unresolved; the three documented limits above are the accepted decisions.

## The PRs

- [#3023 — protect permission revocations from power loss](https://github.com/scode/farhelm/pull/3023/changes), draft.
- [#3025 — keep retry approvals with the requesting host](https://github.com/scode/farhelm/pull/3025/changes), draft.
- [#3028 — qualify macOS database durability](https://github.com/scode/farhelm/pull/3028/changes), draft.
- [#3031 — document recent conversation loss after power failure](https://github.com/scode/farhelm/pull/3031/changes),
  draft.
- [#3034 — document full-disk loss of a Resume offer](https://github.com/scode/farhelm/pull/3034/changes), draft.

## Checks run, reused and skipped

Both runtime commands ran through `scripts/record-test-run.py` in development nextest mode, with pinned nextest 0.9.148,
four global slots and zero retries. Together they passed 126 tests. Their retained source records, JUnit reports and
output are complete, with output EOF and no omitted output, failures or runtime `SKIPPED` notices.

- `cargo nextest run -p farhelm-supervisor -p farhelm-helm --lib -E 'test(db::tests::) | test(store::tests::trust_revocations_) | test(store::tests::browser_permissions_) | test(store::tests::hosts_start_asking_) | test(store::tests::rotation_) | test(store::tests::preferences_)'`,
  recorder `7ef912e4-d021-46a6-b061-36cb3073dce3`: 13 passed. These cover the existing database durability helpers,
  host-permission behavior, token rotation and the new regression checking all five revocations, consent clearing and
  ordinary preference durability. No separate `preferences_`-named test matched; broader preference-unit coverage is not
  claimed. Tmux was marked unused because these selected database units do not need it. The tests observe SQLite's sync
  setting inside the real writes and its restoration afterward; they do not simulate physical power loss.
- `cargo nextest run -p farhelm-helm --lib -E 'test(agent_requests::tests::)'`, recorder
  `1fad4b6e-8cc4-4e08-9b03-fc803211ba20`: all 113 agent-request tests passed, including equal-number/distinct-entry
  retry separation and both creating request paths. Required pinned tmux 3.7c was present and matched the recorded
  substrate. The regression models reused numbers with real stores; it does not reproduce a physical power cut.
- `cargo clippy -p farhelm-supervisor -p farhelm-helm --all-targets -- -D warnings` passed for the first fix;
  `cargo clippy -p farhelm-helm --all-targets -- -D warnings` passed for the retry-key fix.
  `cargo fmt --all -- --check`, dprint on changed Markdown and `python3 releasing/check-changelog.py format` passed. The
  isolated pinned interpreter's `python -B scripts/check-test-sleeps.py` check inspected 412 delays, with zero missing
  reasons. `python3 scripts/check-agents-md.py` reported zero errors and three pre-existing reference warnings.
- Reused after later specification and ledger-URL amendments: the 13-case run's reviewed code on base `f67868fb` (source
  fingerprint `d5d0e55c4e2a015eb3fe40efc7d55dca2ddb84738423b1ef14ce5825cf0adb39`) and the 113-case run's reviewed code
  on base `87987f01` (source fingerprint `52da4b9a56e8247bec557e8715649d7233f4789e14c5104894cd02423a5753a5`). Those
  amendments changed no tested code. The stack was carefully rebased onto `397c3bd0`. The complete upstream diff through
  `7b826e40` contained unrelated test fixtures, CSS, planning and bookkeeping changes; none changes the storage or
  request-key dependencies examined here. The only conflicts were adjacent independent removals from the feedback index.
  Each resolved PR removes only its own entry, retaining the already-landed removals. The three changed source files are
  byte-identical across the rebase, and per-PR comparisons found no other patch changes. Fresh dprint checks passed on
  the rebased Markdown; the existing runtime, lint, Rust formatting and test-delay results still cover the final
  executable changes, so no additional runtime run was warranted.
- Skipped: the full Rust workspace, browser suites and desktop runtime suites. The executable changes are confined to
  database write durability and retry-approval ownership, which the focused runs cover; no UI behavior, layout or
  desktop lifecycle changed. The three specification-only units received dprint checks without additional runtime runs.
  No physical power-loss or Mac drive-cache experiment was performed.

## Review gate's outcome

Each of the five PRs passed its required single fresh-context gpt-6.1-sol review at high effort, with no findings. The
changed code, tests and specification text received separate documentation passes. All commit and PR wording passed
fresh cold reads. The small adapter around the existing durability helper also passed a separate scope check; it
introduces no new persistence mechanism.
