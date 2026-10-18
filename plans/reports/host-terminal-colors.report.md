## What this was about

The original TODO asked for a terminal background color per host. You chose to keep the terminal background because
agents draw their own colors there. You asked for the local host to have the same icon and color choices as remote
hosts, red as a choice for every host, and an optional shared setting that colors each terminal's thin frame to match
its host.

The local host now starts with its familiar red laptop but can choose any host icon and color. Red is available to every
host. The Settings switch starts off; when enabled, it colors every open terminal's frame from its host icon color. A
host with the default color uses the ordinary foreground color. Changing a host's color updates an open session at once,
and changing the switch updates the current client at once. Other open clients pick up the switch when they reload. The
terminal background, text palette and size do not change.

## Things you should know

The running web and WebKit render has no separately painted dark margin around the terminal: the pane, terminal and
xterm viewport use the same gray surface. The optional frame uses the terminal's existing four-pixel padding. When the
switch is off, it paints no frame and keeps the original appearance. This follows the plan's fallback for a margin that
cannot be colored separately.

A remote host can now choose the same red laptop mark as the local host. The locality label and hover explanation still
distinguish them.

The [Manage hosts guide](/docs/using/manage-hosts/) explains the local appearance choices and frame switch.

## Open questions and possible follow-ups

None.

## The PRs

- [#2266: local host icon and color choices](https://github.com/scode/farhelm/pull/2266/changes) — draft head
  `62240f64593c914360c2e019be0cd1b7669dfb25`.
- [#2269: host-colored terminal frames](https://github.com/scode/farhelm/pull/2269/changes) — draft head
  `508893f05f9a98094e05c36276384f326442ff45`, based on #2266.

## Checks run, reused and skipped

All runtime commands below ran through `scripts/record-test-run.py`. Rust used the pinned tmux with four nextest slots
and zero retries; browser runs used the pinned tmux, one worker, zero retries, Chromium and WebKit. The UI JavaScript
run used `--tmux none` and Node's default discovery. Commands below give the test selection, not the recorder's full
child argument list; manifests retain configuration, reporter and private output paths.

- For #2266,
  `cargo nextest run -p farhelm-proto -p farhelm-helm -E 'test(host_appearance::) | test(schema_44_migrates_and_persists_host_appearance_words) | test(schema_58_opens_local_appearance_and_preserves_remote_words) | test(appearance_route_validates_stores_and_announces_only_changes) | test(a_migrated_database_matches_a_freshly_created_one)'`
  passed six selected cases (`cfac866e`). `node --test` in `crates/farhelm-ui/js-tests` passed 294 (`d4d18146`).
  `npx playwright test` selected `terminal-multihost.spec.ts`, `sidebar.spec.ts`, `host-picker.spec.ts`, and
  `terminal.spec.ts` with
  `-g 'host-appearance|host-list-states|selecting a session leaves|switching sessions directly|a row with unbroken|compact hides|a local session.s host line|host picker supports|quick switcher draws|matches Ghostty'`;
  all 22 passed, 11 per engine (`581064f7`).
- For #2269,
  `cargo nextest run -p farhelm-helm -E 'test(=store::tests::terminal_frame_color_migration_and_sparse_round_trip) | test(=store::tests::a_migrated_database_matches_a_freshly_created_one) | test(=store::tests::the_preference_row_merges_per_field_and_null_clears) | test(=preferences::tests::the_preference_routes_read_and_merge_one_shared_row)'`
  passed all four selected cases (`4cf9a155`).
  `npx playwright test 'terminal\.spec\.ts|settings\.spec\.ts' -g 'host-colored terminal frame|matches Ghostty|settings keeps both close controls|resize reaches'`
  selected eight cases. The first run passed surface and resize on both engines but failed frame and Settings on both
  (`02707569`): an overbroad frame selector and a stale Settings count. After those corrections, the same two spec files
  with `-g 'host-colored terminal frame|settings keeps both close controls'` passed frame on both engines but failed
  Settings on a stale tall-viewport expectation (`f05f7382`). The Settings spec with
  `-g 'settings keeps both close controls'` then passed on both engines (`ac30f58d`); with
  `-g 'settings dismisses a row menu'`, its row-menu alignment and Escape case passed on both (`55ee2b79`). Both failed
  run records remain retained; they are not counted as whole-run passes.
- The following unrecorded source checks passed separately for #2266's pre-rebase working tree over `e6dc93c5` and
  #2269's pre-rebase working tree over first-PR head `1091bd8d`: `cargo build`, a Dioxus 0.7.10 release web bundle,
  `cargo clippy --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, the isolated `python -B scripts/check-test-sleeps.py` (303 delay calls, none
  unannotated), and `bun run build` in `website`. These were run after each PR's final product edits; #2269's
  browser-test corrections came before its final Clippy, desktop, format, dprint and source checks. After the rebase,
  Rust formatting, dprint and `cargo check -p farhelm-helm -p farhelm-ui` passed on the final stacked tree, heads
  `62240f64` and `508893f0`.

Reused: #2266's Rust and JavaScript records capture source over `e6dc93c5` with fingerprint
`5fe3d5d8839e85f29874e903c537968f696829ed7d7593bb66f41798b494b728`; its browser record captures a later source snapshot
with fingerprint `b30f5071cc1aec55bd7eecc20b7bbbd022b3ac89a19ac680cd80c009c57d72a7`. #2269's Rust record captures source
over first-PR head `1091bd8d` with fingerprint `048eba6ed504d89c487af00f7bb046630e4d5371419f64796127a0d420d5f4db`; its
final corrected browser records share fingerprint `287eaa4a64534877b920e59dc99ee3d67a7ff4544d5b12780ac90220111d78fc`.
Recorder manifests retain the tracked diffs. Comparing those diffs with the pushed heads showed the passing test
assertions and affected feature code unchanged by the rebase. The rebase added prompt/approval behavior in separate code
and combined two distinct statements in SPEC.md; this leaves the pre-rebase lint, desktop, build, website and source
checks applicable to their respective code and docs. The post-rebase helm/UI compile covers the shared crate on the
final tree.

Skipped: the full Rust suite because the selected storage, route and appearance cases cover the changed contracts; the
full browser suite because the selected two-engine cases cover host marks, Settings and terminal geometry; desktop
runtime because the same UI code passed desktop compilation and its changed interactions were exercised in the web
renderer; CI because it is on demand and adds no uncovered check for this diff.

## The review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort reviewed each PR. The first review found one
documentation mismatch in the host Settings dialog header, which was corrected. The second review's follow-up caught a
stale Settings test count after the browser run revealed the selector problem; both were corrected and the reviewer
confirmed no finding remained. Both reviews included the complete test-authoring checklist. The commit and PR wording
passed separate cold reads.
