# What this was about

Quick launches skipped New and its name field. They now open Rename over the new session while its agent is already
starting. The name is selected, so typing replaces it; Enter without a change, Escape or Cancel keeps it. Closing the
dialog returns to the new terminal, and typing the name does not reach the agent.

# Things you should know

This applies to the bolt shortcut, number shortcuts, combined sequences and the quick switcher's Quick launch row,
including templates that supply a name. A launch that opened New gets no second naming prompt, even after a refusal or
YOLO question followed by a successful retry. Naming a managed-checkout session changes its title and leaves the
checkout's directory unchanged.

Settings' **name sessions on quick launch** switch, beside **skip session confirmations**, is on by default on both new
and upgraded helms. Turning it off restores direct terminal entry. The choice belongs to the helm; other open windows
pick it up when they reload. The stored-setting change advances the helm database to schema 56; older helms refuse it,
as accepted in the plan.

The Launch templates guide now describes naming and the setting. No preview server was started for this unattended run.

# Open questions and possible follow-ups

No decisions remain. Browser coverage includes Chromium and WebKit; native desktop bridge timing was not directly
exercised.

# The PRs

- #2233: the shared naming setting, default and storage — https://github.com/scode/farhelm/pull/2233/changes
- #2234: naming after successful quick launch, documentation and acceptance checks —
  https://github.com/scode/farhelm/pull/2234/changes

# Checks run, reused and skipped

- `cargo nextest run -p farhelm-helm -p farhelm-ui --lib`, through the recorder with pinned nextest and tmux, four slots
  and zero retries: 1,676 passed, no skips; run `060ca4ba-dcde-4caf-b124-f8c8c0b54e93`. Retained output contains no
  runtime `SKIPPED` marker.
- `npx playwright test 'quick-launch\.spec\.ts'`, through the strict recorder with pinned tmux, one worker and zero
  retries: 64 passed, 32 in Chromium and 32 in WebKit, no skips or failures; run `95f3d26c-9dfd-4c5e-8a42-319aeeb6977b`.
- `npx playwright test 'settings\.spec\.ts' 'terminal-replay-rename\.spec\.ts' 'modal-focus\.spec\.ts' 'ime-composition\.spec\.ts' 'github-checkouts\.spec\.ts' 'session-action-shortcuts\.spec\.ts'`
  covered settings (including ntfy settings), replay rename, modal focus, IME, managed checkouts and session shortcuts
  in both engines, through the same strict recorder policy. Run `dc1ec84b-4373-40a1-ad10-bb91648739e4` recorded 146
  passes and two failures: the existing settings layout test still expected ten switches after this feature added the
  eleventh. Both exact-count assertions were updated, retaining every geometry, isolation, focus and keyboard assertion.
  The exact retry,
  `npx playwright test '/settings\.spec\.ts$' -g 'settings dismisses a row menu, isolates the page and returns focus on Escape'`,
  then passed in both engines, run `91dacba2-1b66-4f62-a149-ebd749ce688a`. No cases were skipped. The failed run remains
  retained and is not counted as an overall pass. One intervening invocation selected no tests because its file regex
  was over-anchored (run `2b17d453-c039-41f0-8022-2e43bf0019c7`); that invocation supplies no runtime coverage.
- `cargo fmt --all -- --check`, `dprint check` for the changed Markdown files, and `git diff --check` passed.
- `cargo clippy --all-targets -- -D warnings` and `cargo check -p farhelm-ui --features desktop` passed.
- `cargo build`, `cargo check -p farhelm-ui`, and `dx build --package farhelm-ui --platform web --release` passed. The
  web build used the cargo-installed Dioxus CLI; an earlier invocation resolved a different `dx` executable and refused
  before building. `bun run build` from the website directory passed its build and internal-link validation for 32
  pages.
- `python -B scripts/check-test-sleeps.py`, using the isolated pinned parser environment, inspected 303 test delays with
  no missing rationale. `python3 releasing/check-changelog.py format` passed.

The first preference/schema selection,
`cargo nextest run -p farhelm-helm -p farhelm-ui --lib -E 'test(quick_launch_naming) or test(preferences::) or test(schema_)'`,
recorded 33 passes and three failures in historical database rewind fixtures (run
`6ae9e22d-ac8b-4df1-a3ff-9d38c8816843`). The helpers retained preference columns absent from the historical schemas they
asserted. Removing those later columns preserved the frozen historical SQL and every assertion; the focused correction,
`cargo nextest run -p farhelm-helm --lib -E 'test(=checkout_config::tests::checkout_open_existing_refuses_older_schema_without_migrating) or test(=store::tests::history_schema_migrations_discard_schema_22_history_while_preserving_the_host_registry) or test(=store::tests::schema_28_preserves_history_without_inventing_repo_intent)'`,
passed all three cases (run `c6f06bc2-9b6e-42b4-9a2e-47cb05a31328`). Both selections used the recorder's pinned
nextest/tmux, four slots and zero retries. The failed observation is retained; it is not counted as a pass or treated as
a flake.

The original runtime checks used the source in #2233 at `38f0711e` and #2234 at `e903db09`. Current drafts are
`8c31408e` and `79f5a55f`: their combined tree differs only in those two settings-test count literals. The focused
browser retry exercised that correction; the earlier passing cases, units, builds and static checks still cover the
unchanged product source. Main's later changes were plan bookkeeping only, so no product rebase or extra runtime rerun
was needed. The docs result remains applicable because its page content stayed unchanged.

Skipped the full workspace and browser batteries: the touched crates' unit targets and selected browser files cover the
preference and create/rename interactions. No JavaScript asset changed, so a separate asset-unit rerun added no
coverage. No live installation was changed, and no native desktop runtime was exercised here.

# Review gate's outcome

Each PR passed its single fresh-context gpt-6.1-sol review at high effort. The preference-fixture correction passed the
same reviewer's follow-up check, as did the settings-test correction. Commit and PR wording passed cold reads. No source
finding or scope departure remains.

### Landing

Landed on its own in round 86, as #2233 and #2234, merged bottom-up and unchanged from what the report describes.

#### What landed since the stack was based, and whether it interacted

Besides plan-queue bookkeeping, one change reached main between this stack's base and its landing: #2235, which only
adds a TODO.md entry about supporting agent wrappers. #2234 also edits TODO.md, removing this plan's entry, and the two
did not conflict; the landing rebase was clean and both PRs' diffs are identical to the ones the executor delivered. The
base already held the work this stack builds on: the quick-launch key sequences and partial templates (#2220, #2225,
#2228), the session action shortcuts including Rename from the keyboard (#2218, #2221, #2230), the localhost tunnels
(#2219, #2227, #2229, #2231), initial prompts (#2200, #2203), the outage card (#2198) and the input-method guard
(#2194). The landing monitor read the stack against them, and a fresh-context reviewer did the same independently.
Neither found an interaction:

- A launch sequence ends in a single session creation, so the bolt shortcut, number keys, sequences and the quick
  switcher's Quick launch row all reach naming the same way. A partial template, a refusal, a YOLO question or an
  offline host opens the session launcher before any success, and naming then does not open.
- Naming reuses the Rename dialog that the Rename shortcut opens, and returns focus to the terminal the same way. The
  key-sequence, number-key and session-shortcut listeners ignore keys while it is open, and it already ignores Enter and
  Escape during input-method composition. The stack adds no new text input.
- Naming opens only after the session is accepted, so it does not delay creation or the initial prompt, and the
  launcher's protection against starting a retried launch twice is unchanged.

#### Upgrade

#2233 moves the helm database from schema 55, which the localhost tunnels introduced, to 56 by adding one nullable
column for the naming setting, following the same steps as the tunnel consent column; no other change claims schema 56.
An unset value means naming is on, for new and upgraded helms alike, so existing users will see the naming dialog after
a quick launch. SPEC.md says the switch is on even when the helm never stored a choice; the docs page and the changelog
entry call it on by default and name the Settings switch that turns it off. An older helm refuses the newer database,
which the plan accepted.

#### Main was failing three helm tests, and this stack fixes them

The tunnel landing in round 85 (#2229) added the tunnel consent column but did not remove it from three test fixtures
that rewind a database to an older schema, so those three helm tests have failed on main since then. The tunnel plan's
executor ran only targeted helm test selections, which did not include these fixtures, and the landing monitor reused
those runs instead of running the whole helm library test set, so neither caught it. #2233 removes both the tunnel
consent column and its own column in those fixtures. The report attributes its first run's three failures (run
`6ae9e22d-ac8b-4df1-a3ff-9d38c8816843`) to its own column only; the tunnel column was the original cause on main. The
monitor confirmed all three failing on main before this landing, run `d3e85d00-f298-4382-9774-d115f509aaf2`, and the
whole helm test set passes after it (below).

#### Fixes made during the landing

None.

#### Possible follow-up

Minor: the Launch templates docs page says the naming choice is shared by every window but leaves out that other open
windows pick it up only when they reload, which the Settings help text does say.

#### Checks

- Run now, on main after this landing: the helm crate's library tests through the test recorder, four slots and no
  retries, `cargo nextest run -p farhelm-helm --lib`. 1154 passed and 2 failed, run
  `fb6f2d7d-ca0c-4594-8fc7-bd540cfff8f6`. The two failures were the provisioning tests, which install this checkout's
  own build of Farhelm, and that build was stale (protocol 53, from before the tunnels). After a fresh `cargo build`
  both provisioning tests passed, run `28b5b766-6ce8-4def-b430-b80f22c51a02`. Across the two runs, all 1156 helm library
  tests pass on main; the failed run is retained.
- Reused: the executor's recorded runs listed in the report, including the helm and UI library run
  `060ca4ba-dcde-4caf-b124-f8c8c0b54e93` (1,676 passed), the quick-launch browser run
  `95f3d26c-9dfd-4c5e-8a42-319aeeb6977b` (64 passed) and the settings retry `91dacba2-1b66-4f62-a149-ebd749ce688a`, at
  the revisions the report gives, plus its builds and static checks. The landing rebase changed no code.
- Skipped: everything the report skipped, for the reasons it gives.

Apart from the cause of the three fixture failures, nothing in the executor's report became untrue at landing.
