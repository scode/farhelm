# Shared Replace and YOLO handling

## What this was about

Replace starts a fresh session with the source's settings, then deletes the source. The sidebar and session header each
implemented its retry identity and YOLO confirmation rules. Cancellation fixes had needed repeating across those copies.
The UI also allowed an operation lock to be released manually while its cancellation guard still existed, leaving a trap
for future asynchronous changes.

The maintainer asked for one implementation of those rules with no behavior change, independently checked for every PR.
This stack makes the lock guard its only release owner, decodes launch-refusal evidence once, and puts plain Replace's
retry, captured deletion consent, YOLO question and request sequence in one controller used by both surfaces. The final
PR removes the corresponding Code cleanup entry from TODO.md.

## Things you should know

The inventory found all 17 manual releases in the session launcher. Each became an explicit guard drop at the same
point, including before the successful-create callback opens naming. The launcher already held the guard across its task
and cancellation on the starting main: the older plan's proposed unmount fix was already present. No separate fix or
changelog entry was invented for it.

The sidebar and header keep their existing separate question instances and shared exclusion: only one can ask about a
given source. Their timing remains different. The sidebar consumes an answer before saving a permanent host preference
and can reopen after failure; the header keeps its question mounted through that save. Their existing duplicate-question
error presentation also remains different. A retry identity stays shared across navigation, and cancelling an old
question cannot clear a newer attempt's identity. Restart and Delete did not join the controller: inventory found
distinct workflows rather than another copy of plain Replace.

Refusal decoding preserves the existing priorities: YOLO consent retains an attempt; a proven rejection retires it; a
stale create precondition retires it; uncertain transport, authentication and response-decoding failures retain it.
Action endpoints continue ignoring the create-only stale marker. Tests cover contradictory and unknown header
combinations as well as controller ownership and question transitions.

The controller cutover is one coherent PR for both surfaces, with decision tests. Splitting the adapters would have
required temporary scaffolding or unused-code allowances. A separate scope review accepted the bounded two-surface
design. No new global store, generic action framework, protocol, schema or product behavior was introduced.

## Open questions and possible follow-ups

None required to finish this plan. The separate planned work on launching guarded from a YOLO question and the broader
Restart-with policy remains outside this refactor.

## The PRs

- [#2496: require guards for UI operations](https://github.com/scode/farhelm/pull/2496/changes).
- [#2498: type UI launch refusal outcomes](https://github.com/scode/farhelm/pull/2498/changes).
- [#2500: share UI Replace handling](https://github.com/scode/farhelm/pull/2500/changes).

## Checks run, reused and skipped

All Rust runtime selections used `scripts/record-test-run.py`, pinned nextest 0.9.148, four slots and zero retries.
These pure UI decision tests require no tmux. Every listed run has complete source, JUnit and retained-output evidence,
no runtime `SKIPPED` marker, and no forced cleanup or truncated output.

- Guard tests: `cargo nextest run -p farhelm-ui --lib -E 'test(ops::tests::)'`, 13 passed, 576 filtered; recorder
  `5c8da59b-9ee4-4938-91ab-f6d616774e13`. Covered the guard PR's reviewed tree, committed as `1c11c493`; commit metadata
  was the only later change to that unit.
- Refusal decoding: the `create_refusal_headers_`, `action_refusal_headers_`, `unknown_refusal_header_`,
  `text_transport_and_authentication_` and existing YOLO-header selections in `farhelm-ui --lib`, five passed, 587
  filtered; recorder `d8e7714c-dbec-4348-bf4c-09848e435fcc`. Covered the outcome PR's reviewed tree, committed as
  `9b09b483`. The older stale-create test did not match the requested filter; the new eight-combination table directly
  covered its stale policy.
- Controller decisions: `cargo nextest run -p farhelm-ui --lib -E 'test(replace::tests::)'`, nine passed, 591 filtered;
  recorder `2d87ef0b-137e-4e72-af9e-1ac4e6eb1760`. Covered frozen controller tree `13c92cc2` above `9b09b483`.
- Browser integration: `npx playwright test 'replace\.spec\.ts' 'yolo-guard\.spec\.ts'` through the recorder, 62 passed
  (31 Chromium, 31 WebKit), no skips, failures or retries; recorder `4b4ee801-2c2d-4553-86dc-1b1ff044e3ae`. Three
  workers drove three private stacks with the exact pinned tmux 3.7c binary. Complete report, source and output, EOF
  observed, no forced cleanup or truncation. This covered the same frozen controller tree as the nine decision tests;
  the final `02c6ec2f` commit changes metadata only.
- Static/build checks passed: `cargo fmt --all -- --check`; `cargo clippy -p farhelm-ui --all-targets -- -D warnings`;
  `cargo clippy -p farhelm-ui --lib --target wasm32-unknown-unknown --features web -- -D warnings`; TODO.md formatting;
  CLI and fixture builds; pinned Dioxus 0.7.10 release web build. The isolated `python -B scripts/check-test-sleeps.py`
  passed with 307 delays inspected and zero missing rationales. These check formatting, native/web compilation and the
  changed tests' delay annotations.

Earlier checks are reused per unit because subsequent units leave the operation-token implementation unchanged; refusal
decoding remains unchanged by the controller cutover. The intervening main through `842b03df` changes host-uninstall
refusals and queue reports. Its full product/spec diff was inspected: it does not change the session controller, refusal
headers, retry or consent contracts. No integration correction or additional runtime run was needed. The stack remains
based on `50cd9011`; this unrelated main movement does not require a source rebase before delivery.

Two initial web-build invocations selected a different tool named `dx` on PATH and failed before building. Explicitly
selecting the pinned Dioxus executable corrected that invocation. Two early source-only sleep checks exceeded their
bounded discovery time while compilation was busy; later quiet checks completed. Those incomplete attempts are not
counted as passes or runtime flakes.

The full workspace, desktop, installer, provisioning, website and browser batteries were skipped: this stack changes UI
decision ownership, not those subsystems. The selected browser specs exercise the actual Replace/Replace-with and YOLO
integration risk; Restart/Delete request workflows remain unchanged, and Restart-with's refusal conversion is covered by
the decoder truth table. CI runs only on demand and was not dispatched.

## Review gate outcome

Every PR passed two independent fresh `gpt-6-astra` reviewers at high effort: general adversarial correctness/design
review, and a separate review restricted to observable behavior differences. All six reported no findings or no
observable difference. No bug-fix exception was used. The controller's scope review and all three commit/PR-message cold
reads passed.

### Landing

Landed as #2496, #2498 and #2500, merged bottom-up, with the same content the report describes, rebased onto this
round's main. It was the first of five plans in this landing round, which stacked all five and checked them together
before any merged: this one, connection-stall-phase, never-started-launch-cause, yolo-confirm-launch-guarded and
host-changed-update-warning.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained uninstall-refusal-wording (helm uninstall refusals), helm-ingress-types (typed
request parsing in the helm, and tighter rules on how the helm reports errors that came from a supervisor) and
create-reservation-retention (a create's retry key is freed when its session is deleted, and a refused create keeps its
refusal for 24 hours). A fresh-context reviewer and I read them, and the other four plans of this round, against this
one.

- The UI still reads the same refusal headers from the helm, and the landed changes only alter when the helm sends them.
  This plan's decoding keeps its priorities: a YOLO question keeps the attempt's retry key, a proven rejection or a
  changed target drops it, and an uncertain outcome keeps it.
- yolo-confirm-launch-guarded, which lands later in this round, moved the session launcher's whole submit code into a
  new shared entry point, while this plan changed that same code in place (every manual release of the operation lock
  became a guard drop, and refusals became typed outcomes). The landing merged the two; see the Landing section of
  yolo-confirm-launch-guarded's report. The launcher keeps every change this plan made to that code.
- The other three plans in the round do not touch the sidebar's or header's Replace, the operation lock or refusal
  decoding.

#### Fixes made during the landing

None to the plan's code. Its TODO.md removal conflicted with entries main had removed in the meantime; the landing kept
both removals.

#### Possible follow-ups

- The TODO.md entry about Restart with and the session launcher drifting apart still calls this plan "queued" and
  describes what it would do. Now that it has landed, that sentence is out of date; possible follow-up: reword it.

#### Checks

Together the five plans change the session launcher and Replace in the UI, the helm's connection attempts and host
updates, and the supervisor's launch startup and terminal handling, about 8,900 added and 2,200 removed lines, including
changes to five browser spec files; that reach is why this round ran the whole Rust workspace and the full browser
suite. All of the following ran on the five plans stacked together on this round's main, including the landing's
conflict resolutions:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, the website build (a docs page
  changed), the debug build, the release web build of the UI and `scripts/check-desktop-assets.sh` (which also builds
  the desktop app): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `3907499f-e2bc-4ba3-9a83-ce6526597d54`: 4078 passed, none failed; 40 tests
  were left out by the suite's own filters. Its retained output is complete and has no runtime `SKIPPED` message, so the
  real systemd-scope and SSH tests these plans rely on ran.
- `cargo nextest run -p farhelm-ui --features desktop`, run `63f71d93-80d2-464c-841e-4d7823a61c33`: 773 passed.
  `node --test` in `crates/farhelm-ui/js-tests`, run `e320936a-a430-48b7-b45b-e8a5b2ef6f57`: 328 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `3157847c-bee9-4df2-b327-1137c87e135f`: 2270 passed, 14 skipped by design (WebKit
  clipboard cases and tests that need a real signed-in agent or run on one engine only), none failed or flaky; the only
  runtime `SKIPPED` messages are those WebKit clipboard cases.
- Skipped: the workspace and desktop doctests (no documentation example changed), the desktop smoke test (the desktop
  app crate is unchanged; the UI's desktop feature is covered by the check and tests above), the native Mac app (not
  runnable here) and the installer (unchanged).

The report's checks ran on a main from before helm-ingress-types and create-reservation-retention landed, and it skipped
the full Rust runtime suite and the full browser suite; both ran this round, above. Nothing in the executor's report
became untrue at landing.
