# Typed text from hosts and agents

## What this was about

Host- and agent-supplied text can contain invisible or direction-changing characters that disguise what a person is
reading or approving. Farhelm already escaped most such text, but plain strings let a new display omit that step. The
UI's own decoded reply fields now carry a type that cannot be interpolated or implicitly turned back into a string. Each
consumer must explicitly choose presentation or the original bytes.

The inventory covers session labels, references and metadata; notifications; host registration and connection evidence;
folder and launch history and browsing; repository associations and checkout previews; saved template bindings, wrapper
words and selection history; setup plans and progress; SSH connection help; and desktop tunnel host labels. Original
bytes still serve routing, requests, copies, searches, comparisons, editor seeds and serialized snapshots. Direction
isolation remains in place. Module documentation explains both the enforcement and its limits.

## Things you should know

Three separate fixes accompany the conversions:

- A desktop startup refusal now exposes hidden characters in the supervisor's build and installation identities. It
  preserves app-authored instructions and line breaks.
- A shortened folder label no longer begins partway through a generated escape marker. Ordinary printable names,
  including literal marker-looking text, keep their previous shortening.
- Clone now exposes hidden characters in an inherited repository throughout the launcher. Such a selected repository
  stays read-only until explicitly cleared; the displayed markers never replace the raw repository request value.

The initial repository inventory was wrong about validation: discovered repository choices are validated, but a
supervisor's session association can bypass that parser and reach Clone. Review established the full path; the
conversion preserved behavior, and the final separate fix addresses it. No matching escaping item remains open in the
review-feedback queue; the two items named by the plan had already been fixed by its dependencies. The final code PR
removes this plan's TODO entry.

Directly decoded shared-protocol types, including launch templates, approvals, tabs and trash listings, retain their
existing escaping helpers. Finite helm words, helm-minted opaque tokens, numeric host adapters, authenticated global
configuration and local drafts are documented exclusions. The type forces a visible choice; it cannot prove that an
explicit raw display is justified. It adds no implicit Display or string dereference and retains raw JSON and existing
debug/key spelling.

## Open questions and possible follow-ups

There is no blocked decision. Host destination and alias editors still show authenticated user configuration as raw
editable text; no host or agent writer was found, so improving those editors would be separate work. Unknown future
setup status words retain their existing tolerant display because the current producer is a finite helm enum.

Strict desktop all-target Clippy encounters a pre-existing type-complexity warning in an approval test helper. That
helper is unchanged from the base; this plan does not suppress it or add unrelated cleanup. Desktop production lint and
the selected desktop tests pass. WebKit coverage is browser-engine evidence, not a native app or macOS alert run.

## The PRs

All PRs are drafts in one linear stack; no executor merge or ready action was taken.

- [#2465](https://github.com/scode/farhelm/pull/2465/changes): explicit original-versus-display text type.
- [#2467](https://github.com/scode/farhelm/pull/2467/changes): typed direction-isolated text.
- [#2468](https://github.com/scode/farhelm/pull/2468/changes): session labels and notifications.
- [#2469](https://github.com/scode/farhelm/pull/2469/changes): host registry text.
- [#2470](https://github.com/scode/farhelm/pull/2470/changes): connection status and identity evidence.
- [#2471](https://github.com/scode/farhelm/pull/2471/changes): desktop startup refusal fix.
- [#2472](https://github.com/scode/farhelm/pull/2472/changes): history and browse paths.
- [#2473](https://github.com/scode/farhelm/pull/2473/changes): folder-label escape-marker fix.
- [#2474](https://github.com/scode/farhelm/pull/2474/changes): repository and checkout reply text.
- [#2475](https://github.com/scode/farhelm/pull/2475/changes): session metadata.
- [#2476](https://github.com/scode/farhelm/pull/2476/changes): session references and their raw consumers.
- [#2477](https://github.com/scode/farhelm/pull/2477/changes): saved launcher preferences.
- [#2478](https://github.com/scode/farhelm/pull/2478/changes): setup plans and progress.
- [#2479](https://github.com/scode/farhelm/pull/2479/changes): SSH help and tunnel text, enforcement documentation.
- [#2480](https://github.com/scode/farhelm/pull/2480/changes): inherited repository display fix and completed TODO
  removal.

## Checks run, reused and skipped

Runtime checks use the recorder, pinned nextest 0.9.148, four slots and zero retries. Browser checks use one worker,
zero retries and both Chromium and WebKit. Pinned tmux was required except the pure native refusal selection, which uses
no tmux. Complete manifests, source fingerprints, runner reports, output EOF and cleanup were checked; selected outputs
were inspected for runtime substrate skips. None were found in the listed runs; no selected passing case was reported as
an early return for an unavailable substrate. All retained output was complete, with no literal runtime `SKIPPED`
marker. Nextest's selected-out cases are not runtime skips. These selections do not claim native GUI or macOS execution,
and the browser setup backend remains simulated as described below.

The per-unit UI selections below are independent observations, not a sum of unique coverage. They ran against each named
PR's reviewed working tree before publication; subsequent commit description changes do not change executable inputs.
All listed successful runs have zero failures, errors or flaky outcomes.

| Reviewed unit                                   | Recorder run                           | Actual passed / selected out |
| ----------------------------------------------- | -------------------------------------- | ---------------------------- |
| #2465 type and clamp                            | `99306f53-8c63-4cbb-8607-b2158fe6567e` | 16 / 546                     |
| #2467 isolated presentation                     | `04a7954b-b385-45a8-882e-90c0fc2aa7ef` | 55 / 507                     |
| #2468 UI library                                | `f818fe3f-8537-4340-a143-608025a84f93` | 564 / 0                      |
| #2469 host registry                             | `07ad667c-f8ec-4386-80d7-680e21d2db58` | 58 / 507                     |
| #2470 desktop host evidence                     | `f808d22e-a05d-4ad7-b725-ef23f56adfe1` | 54 / 672                     |
| #2471 desktop refusal                           | `b9183d85-c740-4164-93f9-87a795ce794f` | 3 / 724                      |
| #2472 paths/history                             | `219d6e9b-d17f-4d62-a883-1db36cb92fcd` | 60 / 509                     |
| #2473 corrected folder tail                     | `2b31024e-da03-40fd-9393-b3494b637e24` | 8 / 564                      |
| #2474 checkout contracts                        | `ab705df4-0ed7-476a-b21e-99f52110786f` | 16 / 559                     |
| #2475 metadata                                  | `db9e9dc0-ef1e-4165-9609-30f43f4cbf63` | 55 / 521                     |
| #2476 UI library                                | `4558bd5b-f2ca-4f4d-933d-a95789001fff` | 577 / 0                      |
| #2477 preferences and lookup                    | `b2355da2-290d-4eca-8ef7-8c6dbbb08932` | 69 / 511                     |
| #2477 preference write queue                    | `3a5be7c0-5b6d-46fb-9fd7-9d56b931e475` | 21 / 559                     |
| #2478 setup text                                | `23e81f4c-ccea-4acc-ba09-3bb499735e9f` | 25 / 557                     |
| #2479 desktop SSH/tunnel/type/metadata          | `4aa66f2e-5cdd-43ea-abeb-6b7f5a27d8c5` | 5 / 739                      |
| Final repository fix and raw checkout contracts | `c3514388-be90-4146-a445-6c4841199670` | 12 / 572                     |

Commands were `cargo nextest run -p farhelm-ui --lib`, narrowed with `-E` for the named unit; host evidence, refusal and
final connection selections also used `--features desktop`. The full UI runs cover shared construction and
selection/reconciliation changes; smaller units use their existing boundary, rendering, authority and raw-seed tests.
The first preference selection omitted the write-queue module, so the separate 21-test run closes that gap. The
connection selection includes native argv quoting; the peer-fragment refusal regression is covered by the earlier
three-test run, whose desktop implementation and tests are unchanged.

The final browser command selected twelve cases from checkout composition, checkout recents, sidebar, header, launcher
tabs and shortcuts, SSH help and setup-plan specs, on both engines. Run `88fe7a65-f9eb-4cef-90be-b53a717fa673` finished
with 22 passes and two failures, both in the new Clone fixture: Playwright disposed a listing response while its
callback read JSON. End-state captures reached the safe replacement, but this did not make the case a pass. The fixture
now uses the existing independent-HTTP and drained-route pattern, with every product assertion retained. Narrow rerun
`2dc41f17-6243-43c5-b795-f38c803dcfd8` passed the corrected Clone case once per engine. The original failure is
retained, and its 22 unchanged cases are reused separately; there was no single 24-case passing run.

These cover raw preview/retry identity, checkout recents, browse activation and stale replies, folder history, escaped
display versus raw copying, inherited wrapper words, sequence bindings, SSH command copies, setup-plan copies and
remembered session selection. The setup backend is simulated; that case proves UI plan text and copying, not actual
installation. Final unit/browser source was captured over `f45694bb` with fingerprint
`7906e974162c65782314f01904ec394bdec11f056cde02bbde43b768b2654de8`. Corrected browser source was captured over rebased
`43c36cea` with fingerprint `3ea2ad092e9e37d300598bf2bf19dfe4f254f2082fecf6826e6da303c5c264a3`.

The final default UI all-target Clippy passed, as did actual wasm32 web-feature library/bin Clippy and the Dioxus web
release build. Rust formatting and TODO Markdown formatting passed; dprint has no TypeScript plugin, so no TypeScript
formatter result is claimed. The test-delay source check first exceeded its time budget under concurrent compilation;
that incomplete attempt was retained. It passed after compilation settled, and passed again after the fixture
correction: 307 delays, zero without a rationale. Changelog format passed for all three own fragments.

Main advanced once during validation, with the `rustls-webpki` 0.103.16 lock update and an independent release-note
fragment. The whole stack was carefully rebased onto `4e6a17de`; all fourteen existing PR interdiffs were empty and the
final corrected diff was identical before and after rebase. Earlier unit, native and browser evidence is reused because
this plan's production implementation is unchanged and those tests do not exercise TLS certificate verification. The web
normal dependency graph excludes that library, so the web lint/build also remain applicable. The earlier CLI and
fixtures build is reused for the owned plaintext browser stack, which explicitly serves the fresh web dist and uses the
unchanged workspace version. This is not validation of the separate certificate-verification fix. Strict desktop
production Clippy was run again after the rebase and passed, compiling the updated native dependency chain; no unrelated
runtime battery was needed for that interaction.

The earlier session-label browser run `171490e9-3ee5-41d3-ad6a-e518d91fdb75` passed six cases, three per engine,
covering escaped title/notification presentation and raw rename round trips against #2468's source. Those handlers
remain unchanged. The first folder-tail run `1994babf-2de9-47e9-9a37-3f268908e796` passed seven tests but was superseded
after review required preserving literal printable marker text; it is not evidence for the corrected cut.

The first recorded session-reference attempt `db15fbf3-02f0-4a2b-8826-f9d37f40affd` failed compilation before any test:
a local operation gate still required a borrowed String after callers became borrowed raw text. Its retained record has
no JUnit result. The correction preserves owned key bytes; the later 577-test pass is a separate observation. Other
initial static failures were test-only imports, a mistaken launcher executable, a local host adapter and an equivalent
redundant closure; each was corrected and checked. No runtime assertion failure or latent flake was established by these
attempts.

Reuse is per command: #2465's runtime survives its test-only import move; #2474's executable checks survive removal of
three false comments; #2477's 69-test run survives replacing an equivalent closure with its function. Narrow sleep
checks were reused only where later corrections changed production borrowing or equivalent syntax, with no test or delay
edit. The native refusal implementation remains unchanged after its fix. Earlier full-library passes are supporting
evidence, not a claim that all later source was retested by them.

Default UI all-target Clippy, Rust formatting and test-delay source checks were applied at each conversion; native
production lint additionally covered the host and connection configurations. Fragment formatting covers all three fixes.
Documentation-only corrections reused executable evidence. No workspace battery, installer, release, container
provisioning, native GUI smoke or CI dispatch was added: this plan changes UI text boundaries and pure presentation, and
targeted unit/browser evidence addresses their concrete risks. The live install was untouched.

### Exact command selections

These are the recorded child commands, in runtime-table order, followed by the earlier, combined and corrected browser
selections. Rust commands ran through `scripts/record-test-run.py --runner nextest --kind development`, with the pinned
runner, four slots and zero retries; tmux was `required` except the three pure native refusal tests (`none`). Browser
commands ran from `e2e/` through `../scripts/record-test-run.py --runner playwright --kind development --tmux required`,
with the existing one-worker, zero-retry policy on both engines. The final combined timeout was 900 seconds and the
corrected two-case timeout 300 seconds. Selection labels do not substitute for these arguments.

```sh
# 99306f53-8c63-4cbb-8607-b2158fe6567e
cargo nextest run -p farhelm-ui --lib -E 'test(peer::tests::) | test(clamp_title) | test(the_back_off) | test(menu_label) | test(titles_and_folders_render) | test(bell_label)'

# 04a7954b-b385-45a8-882e-90c0fc2aa7ef
cargo nextest run -p farhelm-ui --lib -E 'test(peer::tests::) | test(hosts::tests::) | test(skew::tests::) | test(approvals::tests::running_prompt_card) | test(approvals::tests::a_command_launch_card)'

# f818fe3f-8537-4340-a143-608025a84f93
cargo nextest run -p farhelm-ui --lib

# 07ad667c-f8ec-4386-80d7-680e21d2db58
cargo nextest run -p farhelm-ui --lib -E 'test(hosts::tests::) | test(list::shared::tests::) | test(the_helm_host_list_fixture) | test(host_alias_decodes) | test(provisioning::tests::run_continuity)'

# f808d22e-a05d-4ad7-b725-ef23f56adfe1
cargo nextest run -p farhelm-ui --features desktop --lib -E 'test(hosts::tests::) | test(peer::tests::) | test(the_helm_host_list_fixture) | test(every_host_phase_decodes) | test(a_refusing_local_supervisor) | test(native_refusal_commands_escape)'

# b9183d85-c740-4164-93f9-87a795ce794f
cargo nextest run -p farhelm-ui --features desktop --lib -E 'test(native_supervisor_refusals_escape_only_peer_fragments) | test(a_refusing_local_supervisor) | test(native_refusal_commands_escape)'

# 219d6e9b-d17f-4d62-a883-1db36cb92fcd
cargo nextest run -p farhelm-ui --lib -E 'test(launch_composer::tests::) | test(folder_dropdown) | test(history_and_directory_paths) | test(launch_history_) | test(directory_browse) | test(a_cloned_fields_untouched)'

# 2b31024e-da03-40fd-9393-b3494b637e24
cargo nextest run -p farhelm-ui --lib -E 'test(folder_dropdown) | test(clamp_title) | test(the_back_off)'

# ab705df4-0ed7-476a-b21e-99f52110786f
cargo nextest run -p farhelm-ui --lib -E 'test(github_checkout::tests) | test(fresh_create_body) | test(summary_card::tests) | test(checkout_metadata) | test(same_fresh) | test(fresh_retry) | test(helm_session_list_fixture)'

# db9e9dc0-ef1e-4165-9609-30f43f4cbf63
cargo nextest run -p farhelm-ui --lib -E 'test(session_metadata) | test(host_identity_decodes) | test(host_identity_survives) | test(quick_switcher::tests) | test(list::row::tests) | test(summary_card::tests) | test(shared::tests) | test(prefill_from) | test(a_cloned_fields) | test(header_peer_text) | test(details_bar_folds) | test(helm_session_list_fixture)'

# 4558bd5b-f2ca-4f4d-933d-a95789001fff
cargo nextest run -p farhelm-ui --lib

# b2355da2-290d-4eca-8ef7-8c6dbbb08932
cargo nextest run -p farhelm-ui --lib -E 'test(api::tests) | test(launch_sequence::tests) | test(templates::tests) | test(automatic_selection) | test(wrapper) | test(quick_switcher::tests)'

# 3a5be7c0-5b6d-46fb-9fd7-9d56b931e475
cargo nextest run -p farhelm-ui --lib -E 'test(api::preference_write_tests)'

# 23e81f4c-ccea-4acc-ba09-3bb499735e9f
cargo nextest run -p farhelm-ui --lib -E 'test(provisioning::tests) | test(provisioning_replies) | test(uninstall_notice) | test(update_progress) | test(state_detail)'

# 4aa66f2e-5cdd-43ea-abeb-6b7f5a27d8c5
cargo nextest run -p farhelm-ui --lib --features desktop -E 'test(ssh_help) | test(tunnel_snapshot) | test(typed_peer_text) | test(session_metadata) | test(local_supervisor_refusal) | test(native_refusal)'

# c3514388-be90-4146-a445-6c4841199670
cargo nextest run -p farhelm-ui --lib -E 'test(cloned_repository_field) | test(typed_peer_text) | test(github_checkout::) | test(prefill_from_checkout) | test(clone_checkout_names) | test(a_cloned_fields_untouched) | test(fresh_retry)'

# 171490e9-3ee5-41d3-ad6a-e518d91fdb75 (from e2e/)
npx playwright test 'terminal-replay-rename\.spec\.ts' 'session-action-shortcuts\.spec\.ts' 'notifications\.spec\.ts' -g 'rename exposes peer title|browser Rename chord edits the filtered open session|the list floats, marks new entries'

# 88fe7a65-f9eb-4cef-90be-b53a717fa673 (from e2e/)
npx playwright test tests/github-checkout-composer.spec.ts tests/github-checkouts.spec.ts tests/sidebar.spec.ts tests/header.spec.ts tests/launcher-tabs.spec.ts tests/launcher-template-shortcuts.spec.ts tests/ssh-help.spec.ts tests/provisioning.spec.ts -g 'Clone discloses unsafe inherited repository text without changing its preview identity|ambiguous checkout retries retain the exact original body after a later refusal|structured checkout previews, launches, and reuses a recent as a fresh clone|composer path actions keep typing inert and browse the selected remote host|composer rejects held and queued stale browse activation after destination changes|composer folder dropdown preserves keyboard launch and grouped history|details copy shows escaped peer text, copies raw bytes, and warns upward|agent switch loads that agent.s remembered wrapper prefix|launcher sequences apply from buttons and unselected search text|ssh help displays and copies both registered-host commands|host-setup-layout desktop keeps answers and exact plan text|auto-select remembers the last click, falls back to newest, and attaches'

# 2dc41f17-6243-43c5-b795-f38c803dcfd8 (from e2e/)
npx playwright test tests/github-checkout-composer.spec.ts -g 'Clone discloses unsafe inherited repository text without changing its preview identity'
```

The successful static/build commands below covered the configurations named above. The desktop production command was
also rerun after the dependency rebase. The separate known-failing command was
`cargo clippy -p farhelm-ui --all-targets --features desktop -- -D warnings`; it is not a successful gate.

```sh
cargo fmt --all -- --check
cargo clippy -p farhelm-ui --all-targets -- -D warnings
cargo clippy -p farhelm-ui --lib --features desktop -- -D warnings
cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web --lib --bins -- -D warnings
# from crates/farhelm-ui/, using Dioxus dx 0.7.10
dx build --package farhelm-ui --platform web --release
dprint check TODO.md
# with the isolated interpreter and pinned parser packages from docs/test-sleep-check.md
python -B scripts/check-test-sleeps.py
python3 releasing/check-changelog.py format
cargo tree -p farhelm-ui --target wasm32-unknown-unknown --features web --edges normal -i rustls-webpki
```

## Review gate's outcome

Every PR passed exactly two independent fresh gpt-6-astra/high reviews: general adversarial and no-functional-change.
The latter confirmed preserved behavior for conversions and qualification of all three separate fixes. Findings about
literal folder-marker text and repository provenance were accepted; equivalent-call and fixture corrections were
rechecked by the same original reviewers. No finding remains open, no reviewer was substituted, and no review swarm was
added. Commit/PR wording passed the required cold reads. Independent resume checks reconciled the full logs and
published diffs.
