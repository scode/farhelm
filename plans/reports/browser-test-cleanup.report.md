## What this was about

Browser tests could leave agents running, templates stored, or shared settings changed when setup or an earlier cleanup
step failed. Later tests then inherited the leftovers and failed misleadingly. The maintainer approved twenty small
test-fixture corrections, with one draft PR per outcome and no product behavior changes.

The stack takes ownership before fallible setup, attempts independent releases even after another release fails, and
keeps the original test failure primary. It also isolates the inline checkout fixture's Git commands, compares trash
records with the canonical fixture root, and keeps a fabricated launch refusal on the helm's actual build.

## Things you should know

Every outcome stayed within its complexity gate. The shared cleanup helper was extracted without changing its behavior;
the later changes reuse it. There is no new fixture framework. A session and its working directory still release in
order: if session deletion fails, the possibly running agent keeps its directory. Shortcut cleanup drains intercepted
requests before enumerating sessions created by those requests. Settings cleanup waits for submitted saves to settle
before restoring the prior setting.

The trust-hint browser run exposed a request handler outliving cleanup and reaching context disposal. The correction
drains routes on both owned pages before closing them or restoring state. Its first run failed in Chromium and passed in
WebKit; the corrected run passed both engines. One unchanged baseline observation passed, so this does not establish a
recurrence rate or a confirmed pre-existing flake. The failed run is retained privately.

A temporary sound-cleanup probe initially expected a cleared optional preference to be JSON null; the API omits it. The
corrected probe passed. This was a probe assertion error, not a shipped-product defect. Temporary probes were removed
from the checkout and retained privately. No live installation was changed.

The canonical-path fix was exercised on Linux. It removes the lexical-path assumption that fails when temporary roots
are symlinks; no macOS run is claimed.

## Open questions and possible follow-ups

No maintainer decision or unresolved source finding remains. These are test and fixture changes; no changelog fragment
is required. The draft PRs are for the landing monitor to land in order.

## The PRs

- [#2138](https://github.com/scode/farhelm/pull/2138/changes): pin-test cleanup.
- [#2139](https://github.com/scode/farhelm/pull/2139/changes): independent permission-template deletion.
- [#2140](https://github.com/scode/farhelm/pull/2140/changes): attachment-session setup ownership.
- [#2141](https://github.com/scode/farhelm/pull/2141/changes): confirmation-setting restoration.
- [#2142](https://github.com/scode/farhelm/pull/2142/changes): download-fixture ownership.
- [#2143](https://github.com/scode/farhelm/pull/2143/changes): resumable-header fixture ownership.
- [#2144](https://github.com/scode/farhelm/pull/2144/changes): link-opening preference restoration.
- [#2145](https://github.com/scode/farhelm/pull/2145/changes): sound-test session and preference cleanup.
- [#2146](https://github.com/scode/farhelm/pull/2146/changes): shortcut-test session ownership.
- [#2148](https://github.com/scode/farhelm/pull/2148/changes): listing-test setup ownership.
- [#2149](https://github.com/scode/farhelm/pull/2149/changes): template-test setup ownership.
- [#2150](https://github.com/scode/farhelm/pull/2150/changes): appearance-test restoration and session cleanup.
- [#2151](https://github.com/scode/farhelm/pull/2151/changes): tab-session setup ownership.
- [#2152](https://github.com/scode/farhelm/pull/2152/changes): trust-test cleanup and route settlement.
- [#2153](https://github.com/scode/farhelm/pull/2153/changes): checkout-composer configuration restoration.
- [#2154](https://github.com/scode/farhelm/pull/2154/changes): checkout-settings save settlement.
- [#2155](https://github.com/scode/farhelm/pull/2155/changes): checkout-trash configuration restoration.
- [#2157](https://github.com/scode/farhelm/pull/2157/changes): canonical checkout-trash root.
- [#2158](https://github.com/scode/farhelm/pull/2158/changes): isolated Git fixture commands.
- [#2159](https://github.com/scode/farhelm/pull/2159/changes): same-build host-launch refusal fixture.

## Checks run, reused and skipped

Browser runs used the recorder, pinned tmux 3.7c, one worker and zero retries, with Chromium and WebKit. The following
successful runs cover the listed outcomes; no skips were reported:

The shared invocation from `e2e/` was
`python3 ../scripts/record-test-run.py --runner playwright --kind development --tmux required --timeout 900 --selection '<scope>' --concurrency 'one browser worker; retries 0' -- npx playwright test <selectors>`.
Earlier runs used a timeout suited to their selected scope. Pinned tmux was first on PATH. Exact selectors for the rows
below, in order, were:

1. `'pins.spec.ts|pin-cleanup-probe.spec.ts|sidebar.spec.ts' -g 'pin from the menu|pin cleanup injection|an unanswered view operation|composer holds offered history|composer Enter on a focused recent'`
2. `'custom-permissions.spec.ts|permission-switch.spec.ts'`
3. `'terminal-attachments.spec.ts'`
4. `'delete-precondition.spec.ts|settings.spec.ts'`
5. `'terminal-files\.spec\.ts$'`
6. `'(ui-plan-regressions|yolo-guard)\.spec\.ts$'`
7. `'terminal-link-clicks\.spec\.ts$'`
8. `'sounds\.spec\.ts$'`
9. `'quick-launch\.spec\.ts$'`
10. `'listing-mutation-fence\.spec\.ts$'`
11. `'templates\.spec\.ts$' 'quick-switcher\.spec\.ts$'`
12. `'terminal-multihost\.spec\.ts$'`
13. `'terminal-tabs\.spec\.ts$' -g 'temporary tab helper cleanup failure probe|retargeting an open tab-close question|a displaced view discovers|restarting the agent rebuilds only the agent island'`
14. `'claude-trust\.spec\.ts$'`
15. `'github-checkout-composer\.spec\.ts$'`
16. `'settings\.spec\.ts$'`
17. `'github-checkouts\.spec\.ts$'` (covers outcomes 17 and 18).

| Outcomes | Scope                                                                               | Passed | Recorder run                           |
| -------- | ----------------------------------------------------------------------------------- | ------ | -------------------------------------- |
| 1        | pin case, injected failure, all three extracted-helper callers                      | 10     | `dfbe26ba-3cfc-4379-9513-9774d926262e` |
| 2        | custom permissions and permission switch                                            | 22     | `0dbb8be3-3636-4ac4-9e53-b9c59475e532` |
| 3        | terminal attachments                                                                | 76     | `e4f12359-072a-4a0c-8e78-0011d538ff15` |
| 4        | delete preconditions and settings, including ntfy settings                          | 36     | `6f20d474-164f-415a-8321-3ecb664b255d` |
| 5        | terminal files                                                                      | 20     | `1dc479e4-b43b-4b02-98dc-86eb07f73306` |
| 6        | resumable header regressions                                                        | 26     | `24b4bff1-eaa9-4153-9f64-b4b26fff82c0` |
| 7        | terminal link clicks                                                                | 16     | `d1ec578c-c627-4c3d-aed1-7b553ac38154` |
| 8        | sounds                                                                              | 14     | `b76469ae-e8a6-48e2-967f-2a6d0e5573d3` |
| 9        | quick launch                                                                        | 54     | `70f38914-46a4-4dba-9ee5-00f20690f249` |
| 10       | listing mutation fence                                                              | 4      | `c7e7fdb1-9b7f-418b-a66f-6ebc9720c667` |
| 11       | templates and quick switcher                                                        | 90     | `aeea6d9b-481d-496d-8b4b-2f3929b80a1c` |
| 12       | terminal multihost                                                                  | 100    | `29fd5426-8a14-44c3-a224-6578f7a5ecbe` |
| 13       | retarget, displaced view, resumable restart, actual-helper navigation-failure probe | 8      | `47430598-a8a2-43b8-956f-4b43f644acd8` |
| 14       | corrected trust hints                                                               | 2      | `947f76d6-0d35-47a5-b22c-db9c85f20920` |
| 15       | checkout composer                                                                   | 36     | `e5102774-115a-44fa-812b-e58a744d7811` |
| 16       | settings and ntfy settings                                                          | 28     | `9b54260c-0c94-401b-a0d0-afdadaafbee0` |
| 17–18    | checkout archive cleanup and trash                                                  | 32     | `3e6f5c73-e107-4d23-a5c4-f99c5a70b394` |

The recorded source heads for those rows, in order, were `50d39688b838`, `7c436628f0b0`, `1c3a52bd6c5f`, `e7cc3f3a4650`,
`707a2fd19ee8`, `5147d2367a2f`, `a26bab01b2d9`, `68c0ee681ef8`, `beb0e649687d`, `579dfd60f808`, `719a65af1fda`,
`5c16ea776dbb`, `7d3bb03a0116`, `3646ceb87235`, `879866bff583`, `0d1aa9d41c74`, and `01bed6e9a810`. Each recorder also
captured the tracked and untracked working changes and a complete source fingerprint; the head alone does not describe
an uncommitted fixture change. Later ledger amendments leave the tested behavior unchanged. Outcome 19's Git isolation
is tested separately below rather than inferred from outcome 15's earlier composer run.

Additional uncommitted failure injections verified original-error preservation, continued session cleanup and preference
restoration: `32300076-82c1-4dde-b47c-cc96c7d0bb96`, `a55570f2-07d8-45c3-89cd-c187ae078063`, and
`6f8753dc-7867-4ed2-8a1d-6a9e260f65d5` each passed 2 of 2. The retained failed sound probe is
`8036a334-ce3a-4084-9472-e2f98ea22cde`; its two link-restoration cases passed. The retained trust failure is
`0a517a19-845a-4a86-a068-eedd57efb4e5`; the one-case baseline is `126eae29-0fc1-4822-9f7e-4ba9b15d6ffd`. The pure
failure probe used generic recording with `deno test -A <private-cleanup-failure-probe.ts>`. The corrected sound probe
selected `'cleanup-policy-probe\.spec\.ts$' -g 'sound cleanup attempts later session'`; the shortcut probe selected
`'shortcut-cleanup-probe\.spec\.ts$'`. The failed sound probe ran the whole temporary cleanup-policy spec. The failed
trust run selected the same trust spec as row 14. The baseline used generic recording with
`npx playwright test 'claude-trust\.spec\.ts$' --project=chromium-claude-trust`.

The final ordinary composer and host-picker run passed 48 of 48, 24 per engine, with zero skips and complete recorder
evidence: `2308cf39-c815-4486-ad40-fe94e16ac245`, source head `b361a318eb00`. It covers outcomes 19 and 20 and repeats
the composer coverage after Git isolation. The focused inline-setup experiment also passed 2 of 2 with inherited Git
repository selectors targeting a separate owned repository; its configuration SHA256 remained unchanged. This
intentional environment experiment used generic recorder mode, not the strict browser policy validator: run
`3aa4ef49-88fc-4161-b782-19cfa396007e`, one worker, zero retries, both engines, zero skips or report errors. The
ordinary final run selected `'github-checkout-composer\.spec\.ts$' 'host-picker\.spec\.ts$'`. The generic experiment put
`env GIT_DIR=<owned-repo>/.git GIT_WORK_TREE=<owned-repo> GIT_COMMON_DIR=<owned-repo>/.git` after the recorder's `--`,
then
`npx playwright test 'github-checkout-composer\.spec\.ts$' -g 'inline checkout folder setup' --workers=1 --retries=0 --forbid-only --reporter=line,json`,
with its JSON output directed to a private file.

The final rebase onto `4cc306544635` kept every test change intact. Three feedback-index conflicts were resolved by
keeping both plans' completed-item removals. Incoming Rust portability edits affect different tests; the cleanup changes
do not depend on their fixtures. Incoming notification focus and viewport corrections do not alter the cleanup
contracts. The newly landed wrapped-link hover behavior intersects the link-click spec's exercised path, so a focused
post-rebase link-click run followed fresh application builds. It passed 16 of 16, eight per engine, zero skips, complete
recorder evidence, run `b8981cd6-1115-4d9b-a069-d73549cb2c0f`, source head `7c295178443f`. Other earlier runs remain
applicable after source inspection, since the rebase changes neither their test behavior nor the paths they exercise.
The post-rebase selector was `'terminal-link-clicks\.spec\.ts$'` through the shared strict invocation above.

Fresh `cargo build` and `dx build --package farhelm-ui --platform web --release` (from `crates/farhelm-ui`) passed at
the rebased tip. The cumulative browser-test patch is byte-identical before and after rebase. The isolated test-sleep
checker passed with 301 delays and zero missing rationales. The checker command was
`python -B scripts/check-test-sleeps.py`, using the isolated interpreter. `python3 scripts/check-agents-md.py` reported
zero errors and warnings; `dprint check TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md` and `git diff --check`
passed. The configured dprint plugins do not cover these TypeScript files, so no TypeScript formatting pass is claimed.

Broader Rust, JavaScript, desktop, installer and browser batteries were skipped because this stack changes only test
ownership and fixtures; the selected browser cases and failure injections cover those contracts. The application builds
were prerequisites, not claims of runtime coverage. Earlier matching application builds were reused until the final
rebase introduced the link-hover interaction. No live-install or vendor-agent test was needed.

## Review gate outcome

Each outcome passed the required fresh-context `gpt-6.1-sol` high source review. Findings about cleanup-error masking
and route lifetime were corrected in their original PRs and re-reviewed. Separate fresh Sol medium wording readers
checked commit messages and PR text. The executor inspected the artifacts and changed source, performed a separate
documentation pass, and executed the runtime checks itself.

Private galaxy-brain accounting retains raw CLI usage. Structured records are incomplete for the first nine outcomes;
native-reader and executor usage and exact reported model attribution are unavailable. No total cost or token claim is
made. A native report cold read checks this report before delivery.

### Landing

Landed on its own in round 69, as twenty PRs (#2138 to #2159, without #2147 and #2156, which belong to other work). All
twenty merged bottom-up, unchanged from what the report describes. They change browser tests, two shared test helpers
and the review-feedback bookkeeping that records these outcomes as done; no product code, specification or packaging
changed.

#### What landed since the stack was based, and whether it interacted

The executor had already rebased the stack onto main as it stood after round 68. The only change to main after that,
before the landing, was the delivery of this plan's own report. So the landing rebase changed nothing outside `plans/`,
and the landed tree is identical outside `plans/` to the stack tip the executor delivered. The landing rebase met no
conflicts; the three review-feedback index conflicts the report mentions were resolved by the executor before delivery.

I reviewed the stack against what was already on main, and a fresh-context reviewer did the same independently. Neither
found an interaction. The areas worth a look:

- The wrapped web-link fix that landed in round 68 (#2156, which stopped false warnings when hovering a web link that
  wraps across terminal lines) changed the link hover that the link-click browser tests pass through. The
  link-preference PR here (#2144) only moves that spec's setup inside its cleanup scope; no assertion changed, and
  nothing in it checks the hover warning. The executor also re-ran that spec after rebasing onto #2156, with fresh
  builds, and it passed.
- Within the stack, the shared cleanup helper is created once and never changed afterwards. The one change to the shared
  session-fixture helper (#2143) adds cleanup only when a new session never becomes ready, and leaves the success path
  untouched, so specs tested before that PR still exercise the same code.
- Three spec files are edited by two PRs each. In settings, #2141 and #2154 change different tests. In the checkout
  composer, #2153 changes the teardown and #2158 isolates the test's git commands, in separate parts of one test. In the
  checkout trash spec, #2155 makes sure the checkout fixture is closed last and #2157 fixes how that step matches trash
  records. Because the stack is linear, the report's run for the later PR in each pair (the settings run, the combined
  trash run, and the final composer and host-picker run) had the earlier edit beneath it.

#### Fixes made during the landing

None.

#### Checks

- Run now: none. The landed tree outside `plans/` is byte-identical to the stack tip the executor delivered, where it
  ran the builds, the post-rebase link-click run and the static checks, so no new runtime run would add evidence.
- Reused: every passing browser run the report lists, at the revisions it records, including the post-rebase link-click
  run `b8981cd6-1115-4d9b-a069-d73549cb2c0f` and the final checkout composer and host-picker run
  `2308cf39-c815-4486-ad40-fe94e16ac245`; the report explains why its earlier runs still apply after its rebase. Also
  reused: the executor's builds, test-sleep check, agent-instruction check, `dprint` on the bookkeeping files and
  `git diff --check`, all at the delivered tip. The report's retained failures are failures, not evidence of a pass.
- Skipped: broader Rust, JavaScript, desktop, installer and browser runs, for the reasons the report gives; nothing at
  landing changed that assessment.

Nothing in the executor's report became untrue at landing.
