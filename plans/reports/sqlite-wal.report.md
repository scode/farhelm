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

### Landing

Landed on its second attempt, with no fixes at landing. The helm's and the supervisor's SQLite databases now use WAL
journaling with synchronous NORMAL, so a small write no longer waits for several disk flushes while holding the shared
connection. An OS crash or power loss can lose the last few ordinary commits; the supervisor's creation of the host
identity, whose loss could leave lasting damage, now reaches the disk before the supervisor serves anything, and token
rotation stays fully durable too, because losing one would bring back browser sign-ins the user had revoked. Existing
databases convert on the first open that owns the state directory.

- #2840: the conversion and per-open settings for both databases, the durable writes, tests, the SPEC_impl.md text, the
  desktop app's identity-mismatch message, a changelog entry and removal of its TODO entry.

The first landing attempt (2026-10-30) sent this plan back: a power loss right after a host's very first start could
keep the helm's record of the host identity and lose the supervisor's, which SPEC.md rules out as lasting damage, and
the desktop app's recovery message could lead to corrupting a restored database. The monitor decided then, on your
behalf, that writes like the identity stay fully durable under WAL (the near-term TODO entry "Revisit keeping full
durability for the host identity under WAL"). The redelivery does what the answer asked: the identity write and token
rotation wait for the disk, the helm's own copy of the identity stays an ordinary write because a lost one is learned
again from the supervisor, and the recovery message now says to restore both databases with whatever `-wal` and `-shm`
files the backup has and to delete current ones it lacks.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained UI work, the Tensorlake test scripts and the trash-deletion timing logs
(log-trash-deletions); none touches the database openers, the identity or token rotation. The trash-deletion logging
adds an ordinary database write after a folder is removed; losing it falls within the losses this plan accepts.
tab-output-hangup lands right after this plan in the same round; the two change separate code.

#### Things you should know

- The executor's report says it skipped the whole Rust workspace and the desktop-build tests because you required small
  selections; the answer asked for both. This landing ran both (below).
- SPEC.md treats NFS as supported, and SQLite documents WAL as unsupported on network filesystems. Use from one machine
  usually works; the remaining risk is a filesystem where conversion succeeds but SQLite later cannot use the shared
  `-shm` index file it keeps beside each database, which would fail every open with nothing converting back. It is the
  one place SPEC.md's lasting-damage rule could still apply.
- Going back to an older version, v0.32.0 included, still opens the databases; the switch to WAL is one-way but older
  builds read it.

#### Checks

This round landed sqlite-wal and tab-output-hangup, stacked in that order on main `e9095ef8`. On that stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 327 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, the workspace debug build and the UI's release web
  build: all passed.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `a47ccf43-6220-4fa1-a441-918dd35fd486`: all 833 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `19b4cecc-69ec-47c1-9d7b-1549b0f5d95c`: all 1016 passed. The UI's JavaScript tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `f937cb3a-908c-4912-aa54-16e68f300565`: all 383 passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, no retries, on a machine with rootless Podman, run `c7309da5-bfd5-494f-a120-8ee35001698e`: all 4612 passed, 41
  left out by the test runner (tests marked ignored, such as real-agent captures).
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines, not the usual development machine; each booted the usual three test stacks and ran its share of the suite's
  218 projects through the recorder's generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack; Node 24.16 there, against 24.20 locally). It took 16 minutes. Across the ten runs
  (`0c8864cb-3172-4c93-b299-459fe2be756c`, `8b206f4c-20eb-4e1f-9502-c77dd9f59481`,
  `0beabe85-c643-48b7-a4c0-d5f75e2f686a`, `bc37b0d7-481a-4b8d-b693-76657dab38a1`,
  `7df60eb0-647f-4a3e-b2bf-db779a98a056`, `b4321572-5693-4867-a361-fcdc51e4d2cc`,
  `a3114930-73dc-4590-8467-de75e94d1d93`, `a4a4ad3b-56c1-426f-8377-149f23f29d12`,
  `8ac5a34e-85cd-4f2c-86c5-ac9756d7ba46`, `3419f1ed-b945-43c4-96ab-3403b80ca8c0`; only the two failed runs' records were
  kept), 2846 passed, 14 skipped by design, 2 failed, both on WebKit. Re-run three times each on the same build, also on
  sandboxes, with `-g` and `--repeat-each=3` (runs `f075c442-c5a9-401a-a226-ec07dffc3f8e` and
  `7b4a27da-01e2-4721-aef3-c5c6c201ae50`). "unchanged launcher tab, harness, permissions and default model retain the
  retry › selected Agent Home" (launcher Enter: the Launch button did not take keyboard focus within 5 seconds) passed 3
  of 3; it is a first observation with no known cause, its record is kept, and it is not logged as a flake on one
  observation. "the dialog refuses blank and over-long messages without sending" (the Send feedback dialog) is a
  recorded open flake, but this time it failed on the Send button's background color (dark where light was expected,
  which fits the record's color-scheme hypothesis) and once more in the three runs alone, both new to its record.
  Neither plan touches either.
- Reused: nothing. Not run: the doctests (the stack adds no doc examples), `cargo check -p farhelm-desktop` and the
  desktop smoke tests (only a message changed in the desktop code, and the desktop-feature UI build was checked above),
  the docs website build (no page changed) and the native Mac app (not runnable on Linux).

The report's checks were the executor's selected runs; this landing ran the checks below. Apart from its reason for
skipping the workspace and desktop tests, nothing in the executor's report became untrue at landing.
