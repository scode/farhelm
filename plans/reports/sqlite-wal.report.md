## What this was about

Small supervisor state writes had been observed waiting 0.2–0.6 seconds for disk flushes on a loaded host, holding the
shared database connection throughout. This plan switches both the supervisor and helm databases to WAL with NORMAL
sync, as the maintainer requested. No new performance benchmark was run.

## Things you should know

Only a process holding state-directory ownership attempts the in-place conversion, before schema work. A failed
conversion or a result other than WAL produces a warning and keeps the current mode; a busy conversion can add the
existing busy timeout to startup. The next owning open tries again. Every ordinary read-write connection confirms the
actual mode before selecting NORMAL; a rollback journal retains FULL. Non-owning opens never convert. The separate
checkout-config CLI opener remains unchanged at FULL.

The accepted tradeoff applies to every write: an OS crash or power loss can lose recent commits while preserving
database consistency. Process crashes retain committed changes. Recovery guidance now says to restore both databases and
accompanying WAL/SHM files from the same backup, because committed state may remain in a WAL file after an unclean stop.

Source inspection found these concrete ways that losing recent commits can disagree with files already changed or
success already reported. This is a list of observations, not an exhaustive audit; none of these operations was
redesigned:

- A checkout directory can remain after its allocation or session-membership record is lost. An archived directory can
  already be in Trash while the restored database still names its old location.
- A deleted session can return in the database after its launch files and attachments were removed. A permanently
  deleted trash directory can regain a stale database entry; existing listing code can prune a missing directory.
- An acknowledged session rename or host add/edit/removal can revert. A host removed after uninstall can reappear in the
  list despite the remote installation being gone.
- A successful browser-token rotation and credential revocation can revert, restoring the old token and browser
  credentials. Initial token creation or browser sign-in can return a secret whose saved record is subsequently lost.
  These credential writes are included in the maintainer's explicit acceptance; no special durability exception was
  added.

No transaction-style change, background retry, shutdown checkpoint or ownership-lock change was needed. Both stores use
the same setup. The TODO entry is removed and the implementation specification and changelog record the tradeoff.

## Open questions and possible follow-ups

No maintainer decision is needed to land this work. These filesystem and acknowledged-success examples are consequences
of the accepted power-loss tradeoff. This work makes no measured latency claim beyond avoiding the normal per-commit
flush in WAL/NORMAL. An existing desktop approval-test helper needs a type alias to satisfy the current all-targets
Clippy check; this unrelated cleanup is left for a separate change.

## The PRs

- [#2840 — reduce database commit stalls](https://github.com/scode/farhelm/pull/2840/changes), draft.

## Checks run, reused and skipped

- Database and caller unit modules:
  `cargo nextest run -p farhelm-supervisor -p farhelm-helm --lib -E 'test(/^(db|store|checkout_config|token_control)::/)'`,
  recorder `e36e28af-11e7-4eef-865c-f4379263b7e8`: 361 passed, 2,355 outside the selection. This covers both stores,
  fresh/existing conversion, retained rows, non-owner opens, warnings and FULL fallback on actual SQLITE_BUSY,
  per-connection sync, migrations, token-control callers and the WAL snapshot's successful competing commit.
- Ownership and desktop recovery:
  `cargo nextest run -p farhelm-helm -p farhelm-ui --features farhelm-ui/desktop --lib -E 'test(=tests::a_second_helm_refuses_before_migrating_an_owned_state_dir) | test(=desktop::tests::a_refusing_local_supervisor_ends_startup_with_its_state_and_remedy)'`,
  recorder `927da9ae-1e88-4b7c-917d-61ebc69966ce`: both passed, 2,342 outside the selection. This validates the
  corrected upgradeable fixture and the sidecar-aware recovery message in the desktop build.
- CLI and handoff:
  `cargo nextest run -p farhelm --test checkout_config_cli --test e2e -E 'binary(checkout_config_cli) | test(=create_idempotency::a_retried_create_returns_the_same_session_across_a_supervisor_restart)'`,
  recorder `255f0e96-67aa-483f-8bd0-b579de0a404b`: all four CLI cases and the one handoff case passed, 408 outside the
  selection. The handoff uses a real supervisor and pinned tmux, and confirms that a retried create keeps its session
  identity across replacement. The ordinary fixture-build setup also passed; JUnit counts that setup as a sixth pass.
- All three recorded runs used pinned nextest/tmux, four slots, zero retries, complete source and runner records,
  observed output EOF, zero omitted output and no forced cleanup. Retained output contains no runtime SKIPPED notices.
  There were 368 passing test cases plus the one passing setup. Build and Cargo-lock waiting accounted for most wall
  time; the tests themselves were short.
- `cargo clippy -p farhelm-supervisor -p farhelm-helm -p farhelm-ui --all-targets --features farhelm-ui/desktop -- -D warnings`
  failed on the unchanged desktop-only approval-test helper's `clippy::type_complexity` warning. Direct comparison with
  main confirms that the flagged type is identical and this PR does not touch that file. No suppression or unrelated fix
  was added.
- `cargo clippy -p farhelm-supervisor -p farhelm-helm -p farhelm-ui --lib --features farhelm-ui/desktop -- -D warnings`
  passed, covering the changed production code in all three crates, including the desktop build.
- `cargo fmt --all -- --check`, dprint on changed Markdown, changelog format validation and the isolated test-delay
  checker passed. The checker inspected 327 delays with zero unannotated calls. The fixture assertion needed formatting
  once after review; it was formatted before the successful final check.
- Reused: the database-unit run tested the uncommitted implementation on base `97270039`. Later edits corrected module
  prose and an ownership-test fixture outside that selection; its tested behavior is unchanged. Main's intervening
  typed-launch work changes template validation, keyboard/UI behavior, docs and bookkeeping, with no store-opening or
  journal-policy changes. The clean rebase preserves both plans' separate TODO removals; remaining checks run on that
  combined tree. No older session's runtime evidence is reused.
- Browser/appshot, desktop smoke, installer and whole-workspace suites were skipped: this changes database setup and a
  native recovery string, covered by database/caller tests and the exact desktop text test. No layout or input behavior
  changes, and no new performance benchmark or power-loss simulation is claimed.

## The review gate's outcome

A single fresh-context gpt-6.1-sol reviewer at high effort reviewed correctness, design, language idioms and the
test-authoring checklist. Its P2 finding was an existing ownership test copying only the main database while
schema-rewind writes could still be in WAL. The fixture now closes its final connection before copying and asserts that
the copied schema actually needs migration. The reviewer verified that fix and a stale module-documentation correction;
both findings are resolved. The separate documentation pass and commit/PR wording cold read are complete.
