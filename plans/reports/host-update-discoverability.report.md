## What this was about

Remote-host updates were easy to miss while working in the session list. The Hosts button now shows an upgrade arrow and
the number of hosts needing updates. Optional updates have a dismissible card; a required protocol update has a
non-dismissible banner explaining why that host's sessions are unreachable. Both offer Update for exactly the hosts they
name, or Show hosts.

Settings also has **Update remote hosts automatically**, off by default for new and existing installations. Enabling it
authorizes the helm to perform the same updates with no window open. Its value is shared by clients and survives helm
restarts.

## Things you should know

The existing Hosts panel's **Update all** keeps its broader meaning, including eligible hosts already on the current
build. The new reminders and automatic updates act only on hosts needing an update. Busy hosts, the helm's own machine
and newer builds are excluded from automatic updates. The desktop app's own update setting keeps its existing scope.

Dismissing the optional card leaves the Hosts arrow visible. The card returns for a newly outdated host, a different
observed helm build, or an interface restart. Required updates always take precedence. Both reminders hide while Hosts
is open. Long host lists scroll within the card while its actions remain reachable.

A failed or refused automatic attempt waits for a new host connection or a helm restart before trying again. Turning the
setting off and on does not clear that memory. A refusal before a run starts is logged without a separate failure
surface; manual Update remains available to show its reason. Accepted runs keep ordinary progress and failure in Hosts.
Turning the setting off prevents new admission and leaves accepted work alone.

The helm database advances to schema 68; older helms refuse the new format. This work changes no supervisor schema. The
still-unreleased host-update changelog fragment covers both PRs, and the final code PR removes both source TODO entries.

## Open questions and possible follow-ups

No outstanding product decisions or required follow-ups.

## The PRs

- [#2807 — host-update reminders in the session list](https://github.com/scode/farhelm/pull/2807/changes), draft, head
  `d0402cc2f8a5ef3137d7c5e22e5ed7ad4e58c6a1`.
- [#2808 — automatic remote-host updates](https://github.com/scode/farhelm/pull/2808/changes), draft, head
  `7cdc86bd0629c99936fdc5329d52cb4a52ff1adb`.

## Checks run, reused and skipped

Run for the reminders:

- UI library tests, recorder `11bba136-2b58-43c5-af92-8beb3fc8ab6f`: 809 passed. After the classifier, dismissal,
  build-observation and movement corrections, the focused hosts/list/skew selection,
  `cargo nextest run -p farhelm-ui --lib -E 'test(hosts::) or test(list::) or test(skew::)'`, recorder
  `891f8f0e-a28b-44b9-936c-816e92ac3dc6`, passed 311 cases. Later production changes only relocated the card's markup;
  the browser checks cover that placement.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, recorder `31c41d18-1b6a-4d37-ad83-8d1edd77a31a`, passed.
  The final reminder web build, `dx build --package farhelm-ui --platform web --release`, recorder
  `dbfe076a-2923-40c3-80cb-34b580c58c34`, passed.
- Playwright recorder `aa39f758-b714-4a08-bd43-1862230fdb19` passed 14 cases, seven on Chromium and seven on WebKit:

  ```sh
  npx playwright test 'host-updates\.spec\.ts|provisioning\.spec\.ts' -g 'optional reminder|optional dismissal|required banner|long update names|update all dispatches|update all skips'
  ```

  These cover exact update targets, dismissal and reappearance, required-update precedence, movement of open row menus,
  short-window geometry, keyboard scrolling and the existing Update all behavior. Retained ordinary-card and required
  banner screenshots were visually inspected. A session tooltip obscures the long-name screenshot; its geometry,
  keyboard and action assertions supply that case's evidence.
- The preceding browser run, `4b7052b0-e84e-46d6-9766-1630d0391161`, passed 12 cases and failed the long-name case on
  both engines. An arbitrary minimum on an intrinsically sized row container was wrong; inspection also found that the
  card was nested in the content scroller and its arrow was misplaced. A separate scope review preceded the narrow
  placement correction and the corrected viewport/real-row assertions. The failed run remains retained. Its four
  existing Hosts-panel case passes remain applicable because their data owners and presentation paths did not change.

Run for automatic updates:

- UI preference queue and Settings tests, recorder `e17c2867-2593-4c80-8019-175eef3184f5`: 91 passed, 720 selected out.
  This includes the new default-off, rollback, reconciliation and supersession test and the existing permission fixture
  completed for the new field. UI lint, recorder `18c61752-48f1-4ec0-8507-187c21693f34`, passed.
- Helm lint, `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, recorder
  `904791d3-88d3-4922-9e23-527058f69f28`, passed. Later changes only strengthen a cancellation fixture's precondition,
  correct the test wait helper and rewind three historical database fixtures. The focused native checks compile and
  execute those changes. No production code changed after lint, so its production coverage remains applicable.
- The affected Helm selection,
  `cargo nextest run -p farhelm-helm --lib -E 'test(/store::|preferences::|provisioning::/)'`, recorder
  `2beb4deb-b538-4db6-8fd2-64e26430942c`, ran 439 cases: 434 passed and five failed. Both existing local and SSH
  end-to-end provisioning cases passed. Two new listener tests returned an idle completed view before an update was
  admitted; their wait helper now requires a real run ID. Three historical database fixtures retained the new preference
  column while claiming an older schema, making their migrated column order differ from a fresh database. Their exact
  schema comparisons remain in place. These failed observations are retained, separately from later checks.
- The two listener failures reproduced unchanged in recorder `8e271c2c-5533-4810-8120-de4b00202ced`, with two failures
  and no passes. After correcting the wait helper, both passed in recorder `d4122324-6a85-4bf5-825e-b69f3dda025a`,
  selecting only `automatic_updates_wake_without_a_window` and `automatic_updates_wake_for_a_newly_eligible_host`.
- The three historical fixture failures reproduced unchanged in recorder `d3a336f3-8868-4cd8-9d77-b03fa5cf29a4`, with
  three failures and no passes. Corrected recorder `26d6fb2b-dcd0-4d88-8754-676c201e712c` passed all three:
  `history_schema_migrations_discard_schema_22_history_while_preserving_the_host_registry`,
  `schema_28_preserves_history_without_inventing_repo_intent`, and
  `schema_66_starts_empty_visits_and_preserves_legacy_preferences` in `store::tests`.
- The final affected-helper check, recorder `f2de3edf-156a-4df4-8036-5df127f05789`, passed 242 provisioning cases:

  ```sh
  cargo nextest run -p farhelm-helm --lib -E 'test(provisioning::) and not test(local_provisioning_and_update_preserve_a_running_session) and not test(provisioning_and_update_over_ssh_preserve_an_operable_session)'
  ```

  It covers the corrected completion helper's callers and the automatic-update tests. The two excluded slow cases
  already passed in `2beb4deb`; accepted manual runs have real IDs before calling the helper, and their production
  inputs are unchanged. Unaffected store and preference case passes from that failed command remain observations of
  those cases; the five failures are covered separately above. The broad command itself remains a failed command.
- `cargo build -p farhelm -p farhelm-fixtures`, recorder `8af6b8be-4805-4d34-9bdc-aaf25d5da02a`, passed.
- The final Settings web bundle, recorder `6fbd1e3d-f749-4e39-9344-e83cd82b34d8`, passed. Later edits affect Helm, tests
  and explanatory prose, so this bundle still matches the final production interface.
- Playwright recorder `3df5bd43-c86e-4d61-9df9-7c534a04f3bf` passed 16 cases, eight per engine, with no skips or
  failures:

  ```sh
  npx playwright test 'settings\.spec\.ts|host-updates\.spec\.ts' -g 'automatic host updates default off|reports and reverts a refused auto_update_remote_hosts save|keeps both close controls pinned|navigation keeps mounted panes|dismisses a row menu|optional reminder|required banner'
  ```

  This covers real shared preference persistence, a second client, refused-save rollback and retry, no writes on opening
  Settings, unrelated-field preservation, the complete 19-switch layout, focus/navigation and the default-off
  interaction with optional and required reminders. Wide Chromium and narrow WebKit Hosts settings screenshots were
  visually inspected. All three owned browser stacks cleaned up normally.
- Initial native recorder `144e31b9-541a-4704-b1a4-382f8b1ac406` failed compilation because one existing exhaustive
  preference fixture lacked the new field. It was corrected before the passing UI run. Intermediate native and build
  recorders `daa2c59c-764d-4bae-877f-a59a72ffd3f2` and `d93a3dc6-0837-4f5b-ac13-61317236ba0d` were deliberately
  interrupted before completion when review required another lifecycle correction; neither counts as passing evidence.
  All incomplete and failed records remain private and retained.

Source checks passed: `cargo fmt --all -- --check`, scoped `dprint check` on the specifications and TODO,
`python3 releasing/check-changelog.py format`, `git diff --check`, and the isolated Python 3.12
`python -B scripts/check-test-sleeps.py` (322 delays, none unexplained). Three earlier checker attempts under Python
3.15 exhausted its source-analysis budget and are not passes; using the prescribed pinned parser environment with the
available Python 3.12 completed the check. No product or checker behavior changed. Changelog fragments are excluded from
dprint; their format was checked by the changelog validator.

Runtime runs used the recorder on Linux x86_64, pinned nextest and tmux, four Rust slots or three browser workers, and
zero retries. Final successful records have complete source identity, report counts where applicable, stream EOF, no
output omissions and normal owned-process cleanup. Retained output was inspected for runtime skips.

The stack was rebased from claim base `fdd9e8a9ad9a7ddaa7f9bfd02252a7071b025142` onto
`b82bfa1163cd95b26a31e55853e66c4262b36d7f`. The complete upstream delta contained only queue deliveries and reports, and
the rebased PR1 interdiff was empty. Final fetch still showed that main. PR1 evidence covers its pushed head; PR2 checks
cover the recorded candidate working trees above it. The production interface is unchanged since its final web build and
browser checks; final Helm production is unchanged since its backend build and lint. Later test-only corrections are
covered by the focused checks above. No further runtime check was needed for queue bookkeeping or commit creation.

Skipped: the full Rust workspace and browser batteries, unrelated JavaScript, desktop runtime, installer, release and
live-install checks. Focused store/preferences/provisioning, interface queue and browser checks address the changed
contracts; this work changes no installer, terminal asset or platform-specific renderer behavior. Native desktop
operation is not claimed. No website preview was needed for these incidental specification edits.

## Review gate outcome

Each PR passed the required single fresh-context gpt-6.1-sol reviewer at high effort. PR1 corrections addressed later
helm-build dismissal, row-menu movement and short-window card placement. PR2 corrections addressed opt-out ordering, the
listener-wake fixture premise and cancellation before admission. Fresh own-model scope reviews preceded repeated
corrections; they kept the existing layout and accepted-action ownership boundaries without adding a new controller. The
same required reviewer also passed the test-helper and historical-fixture corrections, preserving the previous
production verdict. All findings were accepted and resolved. Separate fresh readers checked commit and PR wording. The
documentation pass covered every touched file. Both PRs remain drafts for the landing monitor.
