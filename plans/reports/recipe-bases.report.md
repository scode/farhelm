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

### Landing

Landed with three wording fixes made at landing, two in SPEC.md and one in the changelog entry. Every Tensorlake recipe
now chooses a base (Tensorlake Ubuntu, your image or your snapshot). Farhelm checks the base, installs any optional
Ubuntu packages and both Claude Code and Codex on it, and builds the snapshot that new hosts start from. Existing
recipes are migrated, and hosts already created from them keep working as before.

- #3178: the change, its migration, the base requirements check, the shared recipe editor in Settings and Add host, its
  tests, the specification, the user guide's Tensorlake page and the changelog entry.

#### What landed since the stack was based, and whether it interacted

Only plan bookkeeping reached main after this PR's base until the landing, and this round landed no other plan. While it
landed, main gained #3184, which makes the grid terminal component's own browser tests wait for bold fonts; it changes
only that test file, so main differs from the tested tree there and nowhere else, with nothing shared. Before
delivering, the executor rebased twice, across the recent landings: durable host removal (#3171), the launcher's
Custom-words fix (#3174, a pending launch is cancelled when the Custom permission words change), the agent settings
overwrite fix for Tensorlake hosts (#3173), the Tensorlake state-path refusal (#3168, #3170), the Settings → Credentials
wording (#3165) and safety fixes in two maintainer scripts (#3169, #3172). The landing monitor and, independently, a
fresh-context review read those diffs against this one and found no interaction, and its verdict was to land:

- every implementation-specification sentence they added survives this PR's rewrites of the same paragraphs;
- this PR's only change to the helm's saved data is its recipe migration, which nothing else touched;
- it adds no new way of installing files on a host, so a custom base that ships its own agent settings is handled by the
  settings overwrite fix like any other host.

The review also checked the migration for data loss and found none. Old image and snapshot recipes are stored in the
same shape as the new bases, their build history is kept as it was, and a stored recipe the helm cannot read survives
the upgrade untouched, with listing recipes refused the same way as before rather than that recipe silently dropped.

#### Fixes made at landing

- SPEC.md's Tensorlake setup paragraph still said "a reused image must have systemd", but new recipes no longer reuse an
  image unchanged. It now says that every recipe's build checks first that its base boots systemd, and that host setup
  still refuses a sandbox where systemd is not running. That refusal is not new: it applies to rerunning setup on a host
  made from an image an old recipe reused, and those hosts are otherwise unchanged.
- SPEC.md said migrated image and snapshot recipes show **No snapshot yet**. As the report and the code have it, a
  migrated recipe whose earlier build failed or was cut short instead keeps that failure and offers **Retry**. The
  sentence now says so.
- The changelog entry gains a paragraph for the release curator with the caveats an upgrading user can hit. A custom
  base must boot systemd, allow passwordless sudo, be Ubuntu or Debian with apt, and include bash, curl and git, so an
  image that served hosts before can now be refused for new ones. Building also bills a temporary build sandbox, and
  Farhelm installs its own Claude Code and Codex even when the base already has them. Per `AGENTS.md`, "Changelog
  fragments", such caveats go in the entry.

#### Things you should know

- The real Tensorlake checks the plan required ran with the project API key you supplied on 2026-11-08 when you answered
  its block. The executor's report describes the three snapshot builds and confirms that every sandbox, snapshot, image
  and SSH key they created was removed.
- The review noted three possible follow-ups, none of them a change in what this plan intended:
  - A helm whose state folder path holds `%`, `"` or `\` is refused for Tensorlake connections, but a recipe build
    creates (and pays for) its build sandbox before that refusal. Recipes that installed packages already built a
    snapshot this way; now every recipe does. A check before the builder is created would avoid the charge.
  - A base snapshot taken of an existing Farhelm host carries that host's supervisor, so setup refuses every host made
    from it, as before. It now goes through a whole paid build first; the build's requirements check could refuse such a
    base instead.
  - A queued review feedback item still calls a snapshot recipe by its old internal name. Its caveat still holds: a
    snapshot taken by an interrupted build can be chosen as a base and later deleted when that build is cleaned up (the
    report's open questions say the same). Only the name is out of date, and triage of that item can correct it.

#### Checks

These ran on the PR rebased onto main `7b9d4e3d` (its base plus plan bookkeeping only), before the landing fixes:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the UI's web-target clippy
  (`cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`),
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 423 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, the docs website build
  (`cd website && bun install --frozen-lockfile && bun run build`, since the user guide's Tensorlake page changed),
  `cargo build` and the UI's release web build (`dx build --package farhelm-ui --platform web --release`): all passed.
- Through the test recorder (`python3 scripts/record-test-run.py --kind development ...`), with pinned nextest 0.9.148,
  four parallel test slots and no retries:
  - the protocol and helm crates' tests, `cargo nextest run -p farhelm-proto -p farhelm-helm`, with the pinned tmux 3.7c
    required, run `41c2e3cc-9b6b-4ab8-a2f3-a330e7bee063`: all 1810 passed;
  - the UI crate's tests in its desktop build, `cargo nextest run -p farhelm-ui --features desktop`, with no tmux, run
    `964d8a90-2687-455f-8cf9-ccd909245bd4`: all 1098 passed.
- Every browser spec that drives recipes: `credential-delivery`, `credentials-settings`, `tensorlake-recipes`,
  `tensorlake-lifecycle`, `tensorlake-hosts` and `tensorlake-tunnels`, on Chromium and WebKit (`--runner playwright`,
  `npx playwright test` with those six files, the pinned tmux 3.7c required, three workers, no retries), run
  `f0818466-3890-4c71-9acd-bfe2e0a642a5`: all 112 passed.
- After the landing fixes, `dprint check` and `python3 releasing/check-changelog.py format` passed. The fixes change
  only SPEC.md prose and the changelog fragment, so the runs above still apply to the code that landed.
- Reused: the executor's real Tensorlake builds (recorder runs `9fe30eb3-4cbf-42a3-a4d8-92000f26cb25` and, for its
  cleanup audit, `f4c44984-db04-4e2f-8f17-088123d4a572`), which ran on its final head after both of its rebases. Since
  then the landing's rebase changed nothing outside `plans/` and the landing fixes changed only prose, so they still
  cover the code that landed. None of its local test runs were reused.
- Not run: the rest of the browser suite and the other Rust crates, the UI's JavaScript tests, a real Tensorlake sandbox
  and the native Mac app. The change is confined to Tensorlake recipes in the protocol, helm and UI crates (the helm
  runs the base check, the agent install and the snapshot build on the build sandbox), which the runs above cover, and
  the real-cloud builds are reused as above.

Nothing in the executor's report became untrue at landing, beyond the PR's draft state and head: the landing fixes
changed the head, and the PR is merged.
