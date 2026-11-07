# Named credentials for Tensorlake hosts

## What this was about

Tensorlake recipes could point agents at API endpoints, but held their tokens directly and could not use saved GitHub
tokens or agent subscription logins. Replacing a shared token meant editing recipes. This plan adds Settings →
Credentials and lets recipes select credentials by fixed names. Saved values are write-only in the UI and API, with
prominent plain-text storage warnings; the Tensorlake API key is editable from both Settings sections.

The six draft PRs cover storage and Settings, named endpoint tokens, delivery on setup/connect/replacement, GitHub
authentication, Codex subscriptions, and Claude subscriptions. Both original TODO entries are removed. The separate
follow-ups for ordinary/containerized hosts and skipping Claude's first-run setup remain.

## Things you should know

**Existing endpoint tokens in recipes are dropped by the migration.** Users must re-enter them in Settings → Credentials
and select their names in the recipes. The changelog marks this as breaking.

Credentials are per host. Replacing a saved value updates connected hosts and reaches other hosts before their next
session. Changing which credential a recipe selects remains deferred until the host's next connection or setup. A
delivery failure does not prevent connection: Settings reports that the value was saved but could not be delivered, and
the host shows a warning. Retrying waits for the next connection or credential replacement; there is no background retry
loop.

Claude may show its own first-run setup once on a fresh host. The maintainer accepted this in the latest Decision.
Farhelm writes the selected token into Claude's ordinary settings and writes no onboarding metadata. Claude Code 2.1.319
started successfully in an isolated fresh home with a fake token, showed its theme chooser, and identified the
configured authentication source as an OAuth token. **Actual valid-login authentication was not run because no test
credential was supplied.** The fake-token result does not establish that authentication succeeds.

Codex can refresh the copied login itself, so Farhelm rewrites its login file only when the selected saved login
changes. Switching the recipe away from Subscription leaves that file behind; the user signs out in Codex to remove it.
Copies share a rotating refresh token: renewal by one copy prevents another copy from renewing, so the UI recommends one
login per host or an API endpoint, subject to the provider's terms. This note follows the
[Codex authentication documentation](https://learn.chatgpt.com/docs/auth/ci-cd-auth).

GitHub credentials reach new agents, new terminal tabs, Restart and private checkout preparation before their login
shells run. Existing programs retain their environment. Old terminal sessions receive no new injection, and an older
supervisor needs setup or Update. Farhelm-built snapshots include `gh`; reused images and existing snapshots need it for
private HTTPS clones. If it is missing, Farhelm still delivers the token and shows a warning. Selecting GitHub None
removes Farhelm's token file and leaves the host's Git configuration alone.

The incidental documentation change is the Tensorlake sandboxes guide. No preview server was started during this
unattended run. No real Tensorlake sandbox or live installation was changed.

## Open questions and possible follow-ups

No decision blocks completion. The existing TODO entries retain the work to skip Claude's first-run setup and extend
named credentials to ordinary hosts and containerized sessions. Valid-login Claude authentication remains unverified
until a suitable test credential is supplied.

## The PRs

- [#3074: manage named credentials in Settings](https://github.com/scode/farhelm/pull/3074/changes).
- [#3077: select named endpoint tokens in Tensorlake recipes](https://github.com/scode/farhelm/pull/3077/changes).
- [#3081: refresh endpoint tokens on Tensorlake hosts](https://github.com/scode/farhelm/pull/3081/changes).
- [#3082: use GitHub tokens on Tensorlake hosts](https://github.com/scode/farhelm/pull/3082/changes).
- [#3086: use Codex logins on Tensorlake hosts](https://github.com/scode/farhelm/pull/3086/changes).
- [#3126: use Claude logins on Tensorlake hosts](https://github.com/scode/farhelm/pull/3126/changes).

## Checks run, reused and skipped

Runtime commands used the test recorder. Controlled Rust runs used nextest 0.9.148, four shared slots and zero retries;
tests needing tmux used the pinned 3.7c build. Browser runs used three owned stacks, one per worker, and zero retries.
Selected-out tests are excluded from the pass counts below.

### Run in this execution

- The focused `cargo nextest run -p farhelm-helm -p farhelm-proto -p farhelm-ui --lib` selection for delivery, merges,
  Claude/Codex references, modes and setup passed 37 cases, run `cd86db0c-acef-4380-9162-4f46edd582f0`. This covers
  replacement, deferred recipe edits, no-write reconnects, delete/recreate identity, Codex refreshed-file ownership,
  partial success, cancellation and deadline handling. Its original setup-order assertion was strengthened after review;
  the corrected case below supersedes that one result.
- `cargo nextest run -p farhelm-helm --lib -E 'test(claude_subscription_setup_delivers_before_supervisor_startup)'`: one
  pass, run `03327797-75c2-4dde-92e6-d2330969cd08`. A distinct completed Claude-settings upload must precede supervisor
  enablement.
- `cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(host_creates::managed_github_token_is_fresh_for_agents_tabs_and_restart) | test(github_checkouts::a_host_checkout_clone_receives_the_managed_github_token) | test(service::core::host_launch::tests::) | test(github_token::tests::)'`:
  nine runtime passes and successful fixture setup, run `abedd8ca-741b-4612-bafe-530175a9612e`. This covers the rebase's
  material environment and real checkout/agent/tab/Restart integration risk.
- `npx playwright test tests/tensorlake-recipes.spec.ts tests/credentials-settings.spec.ts -g 'modes retain drafts|Settings keeps recipe answers|a managed recipe|credentials keep fixed names|the two Tensorlake key forms|recipe references refresh'`:
  16 passes, eight per engine, run `8c8506b0-98ff-47c4-805f-972109484780`. Chromium and WebKit cover both agents' modes,
  filtered references, draft retention, deletion guards, write-only values and dark/light sticky actions. Both Claude
  selection captures were inspected.
- Matching CLI and release web builds passed.
  `cargo clippy -p farhelm-helm -p farhelm-proto -p farhelm-ui -p farhelm-supervisor -p farhelm --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo check -p farhelm-ui --features desktop` passed.
- Rustfmt, dprint on changed Markdown, changelog format and the isolated test-delay checker passed; 420 delays
  inspected, none missing a rationale. A separate documentation pass covered all touched source and test contracts.
- The isolated real Claude startup/source probe described above passed its narrowed fake-token checks. It was stopped
  afterwards without answering onboarding or sending a prompt.

The first GitHub rebase attempt, run `2f4dec0e-9084-4157-af05-95d574828317`, failed compilation after reading an
intermediate parameter edit and ran no tests. The parameter was corrected and its complete failure record retained
before the targeted retry. The initial Claude compile check also exposed two type/helper mistakes, corrected before the
successful check and runtime runs. These are development failures, not latent-flake claims.

### Reused evidence

The first five drafts were reviewed and validated during the preceding execution. Their saved heads were verified
against GitHub before resuming. The careful rebase audited every intervening upstream diff and corrected GitHub
spawn/environment and checkout-fixture integration; the focused GitHub checks above cover those material changes.
Current core and browser runs cover the subscription additions and their shared paths.

The catalog/schema tests from run `c752b094-65fc-4074-8eb2-f52b9bfa4052` captured HEAD
`9dbdd79403e4b8096aacba4b575c06fc35a0be12` plus the recorded working changes later incorporated in #3074. Its eight
passes remain applicable to unchanged catalog storage and migration behavior. The delivery-warning rendering runs
`3da8e20c-6069-4f05-aeae-c8c543b2bc6d` and `998ecdd4-8f8b-4c0a-a83c-f2593b9b2046` captured HEAD
`02fbd5739ce37b3a962bd6a47237f0b6c06f8015` plus their respective recorded working changes for #3081. Those warning
components are unchanged; the latter run explicitly established both palettes before capture. These are captured
working-tree identities, not claims that the named commits alone contain the tested changes.

The token-free `gh auth setup-git --force --hostname github.com` probes remain applicable because the command and helper
installation are unchanged: run `04bd324e-9176-4fc5-98d2-68380513c41a` captured HEAD
`02fbd5739ce37b3a962bd6a47237f0b6c06f8015` plus working changes; repeat-idempotence run
`b20e7f34-573a-478f-abd9-a41521272de6` captured HEAD `f1539a8d75dee770d9b1b8b9bfe23b71bea0feb5` plus working changes.
Both invoke the installed Ubuntu `gh` 2.45.0 in owned empty homes. Earlier failed runs remain recorded in the plan's
working log and are not reported as passing commands.

The CLI/web builds preceded only the test backend's nonsecret completion-marker correction; the selected browser routes
and real-agent probe do not use that upload operation. The final rebase added upstream bookkeeping and unrelated docs
metadata without changing product bytes. This preserves the applicability of the successful commands above.

### Skipped

Valid-login Claude authentication was not run because no test credential was supplied, as the latest Decision permits.
No real Tensorlake allocation or builder-image test was run because none was authorized. No full battery,
installer/release tests, native desktop runtime or agent inference turn was needed: focused checks cover the changed
contracts, and desktop compilation covers the shared UI's second build configuration. The docs change adds prose without
moving pages, headings or links, so formatting was sufficient. CI is manual-only; no hosted run was requested.

## Review gate outcome

Each PR passed the required single fresh-context gpt-6.1-sol reviewer at high effort. The resumed GitHub integration
received a fresh review after its rebase. The Claude reviewer found that a setup test's generic upload marker did not
prove credential delivery before startup; the marker was made specific, the same reviewer accepted the correction, and
the narrowed runtime check passed. No actionable finding remains. Commit and PR wording passed separate cold reads.

### Landing

Landed with fixes to seven existing tests that the plan's changes broke, and to some wording. Settings now has a
Credentials section with named GitHub tokens, agent subscription logins and API endpoint tokens. Tensorlake recipes
select them by name, and Farhelm delivers the selected values to those hosts.

- #3074: the credential store and the Settings section.
- #3077: recipes select endpoint tokens by name, with the migration that drops saved recipe tokens.
- #3081: delivery when a host is set up or connects, and when a saved value is replaced.
- #3082: GitHub tokens for agents, terminal tabs and private checkouts on Tensorlake hosts.
- #3086: Codex subscription logins.
- #3126: Claude subscription logins.

#### What landed since the stack was based, and whether it interacted

Nothing but plan bookkeeping and documentation landed on main since the plan's base, and nothing else landed in this
round. So the landing's fresh-context review read the stack against the work that landed in the days before it and
changes the same code, where a conflict would not show up as a rebase conflict: the move of agents, terminal tabs and
Restart into new terminals, one-press launch on a suspended Tensorlake host (#3073) and the hosts-needing-attention list
(#3044). None of it conflicts: every new agent start, terminal tab and Restart receives the GitHub token, a
containerized session never receives the host's token, and the helm database's new schema versions do not collide with
main's.

#### Fixes made at landing

- Five existing tests of database upgrades rebuild an older database by removing every later table and column, then
  compare it with the old shape. They did not remove the plan's new credentials table or the new per-host record of
  credential delivery, so they failed at the stack's top. The landing adds both to those rebuilds (in #3074 and #3081).
- The plan renamed an error message from "removing temporary file" to "removing owned file", and an existing install
  test still expected the old wording (fixed in #3082).
- The Add host test of an inline new recipe, in both palettes, expected the Claude and Codex endpoint fields to show by
  default. Each agent's authentication choice in a recipe now starts at None, which hides those fields, and recipes
  gained a GitHub section. The test now samples the GitHub token field instead and expects the extra section (in #3126).
- The Start a session guide page still said Farhelm does not supply `GH_TOKEN` on Tensorlake sandboxes, which this plan
  made untrue. It now says a private repository needs a GitHub token selected in the host's recipe (in #3126).
- Wording, in #3126: "old tmux terminals" in SPEC.md and on the Tensorlake sandboxes guide page now reads "old
  terminals", the term the rest of the documentation uses; SPEC.md's Security bullet said selected values go only to
  those hosts' usual configuration, but the GitHub token goes to Farhelm's own state on the host, so it now says the
  values go only to those hosts; and SPEC.md's Credentials paragraph no longer says twice that recipes select
  credentials by name.

#### Things you should know

- Endpoint tokens already saved in recipes are dropped when the helm upgrades, as the plan's breaking changelog entry
  says. Anyone using recipe endpoints re-enters their tokens in **Settings → Credentials** and selects them in the
  recipe.
- Per the executor's report, a Claude login has been tried only with a fake token: Claude started and named the token as
  its authentication source. Signing in with a real login was not tested, because no test login was supplied.
- When a Tensorlake host connects, the same step now starts its tunnels and delivers its credentials. The
  macos-tunnel-exit plan, which moves tunnels onto the supervisor connection, has to build on that changed step.

#### Checks

This round landed only this plan, rebased onto main `4326cd71`. Before the fixes:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 420 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older text), the docs website build (`bun run build` in `website/`), the workspace debug build and the
  UI's release web build: all passed.
- The UI's JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`), run `b66f77c7-a90c-4288-9803-b961b3c2ea16`:
  all 524 passed. The desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `2a27b6ba-c2fc-47d3-91d4-240549a33f27`: all 1089 passed.
- Every Rust test in the workspace, `cargo nextest run --workspace --exclude farhelm-desktop` (with rootless Podman and
  its offline image present, so the containerized tests ran), run `5e2ee8e4-2a37-496a-a10e-c241b70a74db`: 5722 passed,
  69 skipped by their own design, 6 failed, all in the helm crate and all fixed at landing: the five database-upgrade
  tests and the install test described above.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each ran its share of the suite's spec files through the test recorder,
  `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack; the sandboxes run Node 24.16 rather than the development machine's 24.20), runs
  `a2b1177e-e137-4292-b989-4817e47b9b6d`, `627f4a69-8586-40fa-93f6-395295f760d5`,
  `cb8823f0-6b27-4a82-ad4c-6dc1db4a9d87`, `72b875e3-4efa-4005-a79d-eac2baf40719`,
  `002010e0-6d8e-4592-84ba-131c065ccf31`, `f09f0f74-b1fa-4c59-8e35-4ed721240648`,
  `073e6664-8eea-4f1c-a1fc-98b8564dc4be`, `0571b1fb-89fb-4341-9c86-9558ffc22f4c`,
  `ad9d874e-f09d-4007-bd54-013a5f82ca48`, `95e81060-2a47-464c-ac80-9a26e54c90e1`: 3009 passed, 14 skipped by the specs'
  own design, 7 failed:
  - "Add host pins inline recipe answers and focuses Name", both engines in both palettes (runs `ad9d874e` and
    `95e81060`): the test fixed at landing, described above.
  - "host-settings scrollports stay reachable while saving" (WebKit, run `627f4a69`), which edits an ssh host's
    destination, something this plan does not touch, and "settings saves and clears the all-hosts checkout folder"
    (WebKit, run `002010e0`), which timed out before its cleanup. Each passed three re-runs below. With one failure each
    under a loaded full-suite shard, neither is recorded as a flake.
  - "create-intent-key-rebinds-on-retarget: a moved host is a new intent" (WebKit, run `72b875e3`), a recorded flake
    with the same signature; it also passed three re-runs below.
- After the fixes: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `dprint check`,
  `python3 releasing/check-changelog.py format`, the docs website build, the workspace debug build and the UI's release
  web build: all passed. The helm crate's unit tests, `cargo nextest run -p farhelm-helm --lib`, run
  `e4a78ca8-95b6-4d87-9658-55ac84ecabd0`: all 1552 passed, the six fixed Rust tests among them. On Tensorlake sandboxes,
  with a new build from the fixed stack: the whole Tensorlake hosts spec file, which holds the fixed Add host test in
  both palettes, on Chromium and WebKit, runs `e73a8806-f5b1-479e-95cc-e09f8fc65a03` and
  `b933df18-7386-4d38-9557-25142f1ebf0c`: 11 passed on each engine; and three repeats each of the three WebKit failures
  above (`--repeat 3 --grep ...`), runs `2144e44a-3ab6-436b-886e-9360ea84bd6b`, `d1b99a0b-4db5-4c79-9fd2-e50de2c60877`
  and `c2e23fdd-9233-4189-ba8c-169d52af1917`: all passed.
- Reused for the fixed stack, from the runs before the fixes: the web-target UI lint, the binary lint, the desktop
  compile check, the JavaScript and desktop-build tests, the rest of the workspace's Rust tests and the rest of the
  browser suite. The fixes change only tests, comments, documentation and the order of two module declarations, none of
  which those runs depend on.
- Not run: the doctests (no doc examples changed), the old-terminal browser set (nothing in this plan depends on the
  terminal kind), the grid component's browser suite (no grid code changed), the desktop smoke tests and the installer
  tests (no desktop shell, packaging or installer code changed), the docs website preview (the build above passed; no
  page was viewed), delivery to a real Tensorlake host with real credentials (the sandboxes above only ran the tests;
  the hosts under test were simulated) and the native Mac app (not runnable on Linux).

The executor's report lists only focused test selections. The full runs here found the seven failing tests above, which
those selections did not include; nothing else in the report became untrue.
