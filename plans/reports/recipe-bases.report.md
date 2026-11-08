## What this was about

Recipes previously either installed software on Tensorlake Ubuntu or reused an image or snapshot unchanged. Custom bases
therefore skipped Claude Code and Codex installation. The maintainer chose one model: every recipe selects a base,
optionally adds Ubuntu packages, installs both agents with their official installers, and builds the snapshot used for
new hosts. This is the first prerequisite for the separately planned prepared snapshots and sandbox sessions.

The draft implements that model in Settings and the inline Add host editor, checks the base before installing anything,
and migrates existing recipes. Existing hosts retain their software. Rerunning their setup skips the newly required
snapshot build. The required cloud checks, previously blocked on project access, now pass.

## Things you should know

Old package recipes retain their packages and built snapshot. Old recipes that reused an image or snapshot become that
base with no packages and need one build before adding another host. A migrated recipe with an earlier failed or
interrupted build retains its history and offers Retry, rather than erasing ownership records to display No snapshot
yet; Retry performs the same build.

Changing a base preserves the old built snapshot until a successful replacement is recorded. A failed rebuild leaves it
available for new hosts. Existing hosts are unaffected. Snapshots Farhelm owns for recipes remain unavailable as bases,
because a later rebuild or deletion may remove them.

Custom bases must boot systemd, allow passwordless sudo, be Ubuntu or Debian with apt, and contain bash, curl and git.
The first missing requirement stops the build with its name. Optional packages cannot repair a missing prerequisite.
Both agents are always installed; no agent toggle, mise integration, prepared-supervisor snapshot or per-session sandbox
behavior is included. The Phase 2 TODO stays for the final plan.

Real verification used four sandboxes total out of the authorized 100: one seed and three recipe builders, each with 1
CPU, 1 GiB memory and 10 GiB disk. All three built ripgrep plus Claude Code 2.1.319 and Codex 0.167.0 and produced
completed filesystem snapshots. A separate audit confirmed the saved snapshots came from the recorded builders and all
four sandboxes ended. All four snapshots, the private test image and the registered SSH key were removed. Verification
used an isolated temporary helm; the live installation was untouched. These checks build snapshots; they do not add real
hosts or launch agent sessions from those snapshots.

## Open questions and possible follow-ups

No maintainer decision blocks delivery. Prepared snapshots and sandbox sessions remain separate dependent plans. The
existing snapshot-ownership caveat remains: a snapshot taken by an interrupted builder before the recipe records it can
be selected as a base, then deleted when that builder is swept. This plan preserves the established current/retired
snapshot protection rather than introducing another ownership catalog.

## The PRs

- [#3178](https://github.com/scode/farhelm/pull/3178/changes): recipe bases, migration, requirements checks, shared
  editor, tests, specification, user-guide wording and changelog. One open draft, based on main `0ef65075`, head
  `0320cad5`.

## Checks run, reused and skipped

Run during this resumption on `0320cad5`, with no working diff:

- `cargo build -p farhelm` passed, producing the CLI used for the real checks.
- Recorder-backed private Deno cloud harness,
  `python3 scripts/record-test-run.py --kind development --tmux none -- deno run -A <private-live-check.ts>`, run
  `9fe30eb3-4cbf-42a3-a4d8-92000f26cb25`, passed all three production builds: Tensorlake Ubuntu, a private image
  registered from the owned seed snapshot, and that user snapshot directly. The production Ubuntu requirements check
  passed before installation. Builds ran serially through the public API of the isolated helm; no fake Tensorlake or
  provisioning backend was enabled.
- Independent result and cleanup audit through the same recorder wrapper, child
  `deno run -A <private-cleanup-audit.ts>`, run `f4c44984-db04-4e2f-8f17-088123d4a572`, passed. The read-only access
  prerequisite, run `abfc3983-305e-40ab-9bfa-79a70e6b5b1c`, returned HTTP 200. All three records have complete source
  capture, output through EOF with no omissions and complete cleanup. The private helper scripts passed Deno format,
  type checks and zero-test utility checks; those utility checks are not product-test coverage.

Reused from the original execution:

- Recorder-backed `cargo nextest run -p farhelm-proto -p farhelm-ui -p farhelm-helm --lib -E <recipe-filter>`, run
  `aca3c1ed-481a-4c7d-9cfa-933d235080b3`: 132 passed, 2,584 selected out. The filter covers protocol recipe tests,
  Tensorlake UI tests, helm Tensorlake tests and late SSH-discovery refusal. Its attempted add-host namespace matched
  nothing; that coverage was supplied separately by
  `cargo nextest run -p farhelm-helm --lib -E 'test(/^provisioning::service::tensorlake_add::tests::/)'`, run
  `ba72cacf-babc-4f5a-b676-f25a6b06ced0`: 14 passed, 1,562 selected out. Both used pinned nextest 0.9.148, four slots,
  zero retries and the pinned tmux, with complete evidence and no runtime SKIPPED marker.
- Recorder-backed browser selection
  `npx playwright test tests/tensorlake-recipes.spec.ts tests/tensorlake-hosts.spec.ts tests/tensorlake-tunnels.spec.ts tests/credentials-settings.spec.ts`,
  run `86b51ddb-50f8-442b-9188-2f102be42714`: 50 passed and two WebKit cases failed. A test assertion selected a
  historical builder by reused name. It was changed to join the built snapshot to its builder ID. The failed run remains
  retained; it is not reported as passing. The four affected cases passed on both engines in run
  `3bd1a644-17bd-45f9-a238-66b15b2cb9f7`. The exact history reproduction failed before the correction in
  `cca9fc90-24fb-410a-be6a-b5695bf25649` and passed both WebKit repetitions after it in
  `e2909135-b3d9-4db9-9369-05b9fdedb34a`. An earlier reproduction selected no tests and supplies no evidence. These are
  overlapping selections, not totals to add together. Ordinary browser runs used three workers, zero retries and the
  pinned tmux; the single-engine repetitions used generic recorder mode.
- `cargo clippy -p farhelm-proto -p farhelm-helm -p farhelm-ui --all-targets -- -D warnings`, CLI/fixture and release
  web builds, `cargo fmt --all -- --check`, changed-Markdown `dprint check`,
  `python3 releasing/check-changelog.py format`, and isolated `python -B scripts/check-test-sleeps.py` passed. The sleep
  check inspected 423 delays with no missing rationale. The browser assertion correction required no product rebuild.

The corrected Rust tests cover the frozen `5320bc33` working tree; the add-host tests and application builds cover
`8df824f7`, differing only by specification prose. The corrected browser run covers `db00f3bc`, adding only the identity
assertion and its comment. Both careful rebases preserve that recipe implementation; the latest interdiff from the
original pushed `7074763e` to `0320cad5` is empty. Incoming host-removal durability, launcher identity, maintainer-tool
safety and agent-settings comparison changes were inspected. They preserve the interfaces this recipe patch uses; the
unconditional file installer remains available. The latest combined CLI was built and exercised by the real checks.

Skipped: repeating unchanged local suites, the full workspace/browser battery, desktop application, installer and
unrelated website checks. No material uncovered interaction justified those runs. The user-guide edit is incidental
ordinary prose, with headings and links unchanged, so no preview or website runtime build was started.

## Review gate's outcome

The required single Opus 5.5 reviewer at high effort found no remaining actionable code or documentation findings after
the accepted corrections. Its final code clearance is reused across the unchanged patch. The reviewer explicitly left
real Ubuntu/image/snapshot verification outstanding; those checks have now passed rather than being waived. The required
independent resume check passed, and the existing commit and PR wording retains its accepted cold read.
