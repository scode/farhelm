## Since the last review

PR #2840 remains the single draft PR. Its earlier delivery was stopped before merging because losing a newly created
supervisor identity after the helm remembered it could leave a fresh installation unable to start. The latest decision
requires stronger durability for writes whose loss causes lasting damage, and corrected backup instructions. This round
adds that protection for host identity creation and browser-token revocation, audits the other initialization paths, and
corrects the recovery advice and documentation. The original blanket acceptance of losing every write no longer applies.

## What this was about

Small supervisor state writes had been observed waiting 0.2–0.6 seconds for disk flushes while holding the shared
database connection. The plan switches the supervisor and helm databases to WAL with NORMAL sync for ordinary writes. No
new performance benchmark was run.

## Things you should know

Only a process holding state-directory ownership attempts conversion, before schema work. A failed conversion or a
result other than WAL warns and continues in the current mode; a busy conversion can add the existing busy timeout to
startup. The next owning open tries again. Each ordinary read-write connection confirms WAL before selecting NORMAL;
rollback journaling retains FULL. Non-owning opens never convert, and the separate checkout-config CLI opener stays
unchanged.

Creating a supervisor's host identity now commits at FULL before that identity can reach a helm. The helm records it
independently, so its source must survive once reported. Token rotation likewise commits replacement of the sign-in
token and deletion of browser credentials at FULL before acknowledging revocation. Otherwise a power loss could make
revoked credentials valid again. If WAL conversion fell back to rollback journaling, these protected writes use EXTRA,
which also synchronizes journal removal. Both paths restore the connection's previous setting afterward; ordinary writes
retain NORMAL. No checkpoint worker, retry task or extra transaction protocol was added.

The audit found no further installation-bricking dependency in the examined initialization paths. A helm that forgets
its first-contact record learns the same durable supervisor identity again. Schema migrations are atomic database-only
changes: losing one leaves a complete earlier schema to migrate again. Local-row setup can repeat. Losing a registry
namespace or entry identifier can invalidate a remembered reference or repeat an approval or launch; it does not make
the databases unusable. Initial token creation and browser enrollment can be lost, requiring sign-in again. Those
ordinary lost-tail consequences remain accepted.

The recovery message now says to restore both databases from one backup with the sidecars that backup includes, and to
remove current sidecars absent from the backup before restoring. A leftover journal must not be applied to an unrelated
restored database. The changelog also warns that copying only `.db` files while Farhelm runs can miss recent changes
held in the accompanying `-wal` and `-shm` files.

Other concrete consequences remain: an allocated checkout can outlive its database record; a moved directory can be in
Trash while the restored record names its old path; a deleted session or permanently deleted trash directory can regain
stale bookkeeping; and an acknowledged rename or host registration change can revert. Managed checkouts are not required
to survive power loss, and their identity checks still refuse unrelated replacements. Installation records and prepared
checkout receipts use separate existing file-write mechanisms, which this change leaves alone. Ordinary SQL commits
remain consistent and survive process crashes; the accepted loss concerns OS crashes and power loss.

The original SQLite TODO entry is removed. The separate entry asking the maintainer to revisit the durability judgment
remains, as requested when the landing monitor answered the block.

## Open questions and possible follow-ups

No additional maintainer decision is needed for this implementation. The remaining filesystem and reported-success
losses above are consequences of the accepted NORMAL policy. This is a source audit and a test of the selected SQLite
settings and call boundaries, with no simulated power-loss or measured latency claim.

## The PRs

- [#2840 — reduce database commit stalls](https://github.com/scode/farhelm/pull/2840/changes), draft.

## Checks run, reused and skipped

- Focused library tests:
  `cargo nextest run -p farhelm-supervisor -p farhelm-helm -p farhelm-ui --features farhelm-ui/desktop --lib -E 'test(db::tests::) | test(host_identity) | test(concurrent_first_mint) | test(rotation) | test(durability) | test(checkout_config_snapshot_reads_one_consistent_view) | test(a_refusing_local_supervisor_ends_startup_with_its_state_and_remedy)'`,
  recorder `feb7bd49-2826-4e3e-a22c-02b4603a7b59`: 38 passed, 3,702 outside the selection. These cover the actual
  identity and token write settings, stronger-setting preservation, rollback-journal EXTRA, successful and failed
  restoration paths, identity stability and races, credential-revocation serialization, both store openers, busy WAL
  fallback, the WAL snapshot and the exact desktop recovery text. Pinned nextest and tmux, four slots and zero retries;
  complete source and runner records, output EOF, no omitted output, no forced cleanup and no runtime SKIPPED notices.
  Execution took 1.169 seconds after the build was ready.
- The first identical selection, recorder `669228dd-4633-4a25-a28c-ee66e6a9711e`, also passed all 38 cases, but was
  mistakenly recorded with tmux marked unused. The selection included real terminal fixtures. It was repeated once to
  correct that substrate evidence; the run above is the controlled result. These are 38 unique cases, not 76. Initial
  compilation took 6 minutes 38 seconds; its test execution took 3.022 seconds. No test failure triggered a retry.
- `cargo clippy -p farhelm-supervisor -p farhelm-helm -p farhelm-ui --features farhelm-ui/desktop --lib -- -D warnings`
  passed, covering the changed production libraries and desktop build. `cargo fmt --all -- --check`, dprint on changed
  Markdown, changelog format validation and the isolated test-delay checker passed; 327 delays had zero missing reasons.
- Reused unchanged checkout-config CLI and supervisor-replacement coverage from recorder
  `255f0e96-67aa-483f-8bd0-b579de0a404b`:
  `cargo nextest run -p farhelm --test checkout_config_cli --test e2e -E 'binary(checkout_config_cli) | test(=create_idempotency::a_retried_create_returns_the_same_session_across_a_supervisor_restart)'`.
  Four CLI cases, one replacement case and the fixture build passed on the original PR implementation over `cac2d3bb`,
  fingerprint `e7353f7f5e70ad41890d55e4dc643ffbe3ef8de6b36f3e8eaac48e2ea5b7d39b`. Its retained source, report and output
  are complete, with pinned tmux and no runtime SKIPPED notices. This round leaves the separate CLI opener and
  retry/replacement behavior unchanged; the new identity-write boundary is tested directly in the current run rather
  than inferred from that old result. Upstream changes were inspected before the rebase and introduce no interacting
  store-opening or identity changes. The tested tree equals the pushed PR tree at
  `281554d4194f345c36fc3dd35b69a0bae8dd3f43`.
- The whole Rust workspace, whole desktop suite, browser suite, native smoke, installer and performance benchmarks were
  skipped. The user explicitly required small relevant selections. The affected durability and recovery contracts are
  covered above, with no UI layout or input change. All-targets Clippy was not repeated: the original report retains its
  unrelated desktop approval-test type-complexity warning, while the current production-library check passed. No
  power-loss simulation or proof of absence of every possible lost-tail consequence is claimed.

## The review gate's outcome

A single fresh-context gpt-6.1-sol reviewer at high effort read the full combined PR and latest Decisions, including the
test-authoring checklist. It found no issues in the durability boundaries, fallback, recovery text or changed tests. The
separate documentation pass is complete. Commit and PR wording received a fresh-context cold read.
