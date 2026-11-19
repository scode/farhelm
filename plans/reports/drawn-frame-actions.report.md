# Desktop actions use the state the user saw

## What this was about

The desktop could show one session in its open pane and send a header action to another if the selection changed before
dispatch. A session menu could show an ended agent but send Delete with permission to stop a running one if an update
reached the app just before the press. A key could trigger an action in a frame the app had drawn and had yet to show.
On Wayland, dropping files after moving the pointer could target the spot where the drag entered the window.

The maintainer asked for one rule and one reusable mechanism. The desktop takes the target and the permission to stop an
agent or close tabs from the frame it showed. The session header, interrupted-session card, row menu and environment
Stop and suspend share the presentation check. The app and supervisor still check whether the action can run against
current state. The Wayland backend uses the last drag position even when the motion arrived while it was reading the
file list.

## Things you should know

Restart with and Replace with use the warning shown on the launch button. When the launcher asks separately about
starting a YOLO session, the source warning continues to follow the session. Each affirmative answer uses the permission
shown when the user answered. If the warning was absent at that press, an agent that restarts while the host setting
saves or the checkout preview loads is protected from being stopped. Cancel closes the question without launching.

The recorded decision split the launcher from the menu and card, with about 240 changed lines for the launcher and tests
for both waits and Cancel. The final launcher draft changed 253 lines after excluding the shared prompt helper and
bookkeeping. I kept the extra lines to cover a review finding: Enter on a choice needs to apply the first edit and
submit Restart with in one press.

## Open questions and possible follow-ups

With Skip session confirmations enabled, bulk Delete can stop a marked agent that restarted after its row showed it as
ended. The bar uses the displayed marked rows and reads their current running state when pressed. Applying the same
displayed-state rule to bulk Stop and Delete is a possible follow-up.

## PRs

- Use the shown session at the top of the open pane and on the interrupted-session card.
  [#4176](https://github.com/scode/farhelm/pull/4176/changes)
- Use the shown permission in the row menu and for the interrupted-session card's Restart.
  [#4193](https://github.com/scode/farhelm/pull/4193/changes)
- Bind Restart with and Replace with to the displayed warning through both waits.
  [#4204](https://github.com/scode/farhelm/pull/4204/changes)
- Use the displayed environment state for Stop and suspend. [#4181](https://github.com/scode/farhelm/pull/4181/changes)
- Use the release position for Wayland file drops. [#4183](https://github.com/scode/farhelm/pull/4183/changes)

## Checks

All runtime commands ran through `scripts/record-test-run.py`. Nextest used the pinned runner, four slots and zero
retries.

### Run on the final stack

- `cargo nextest run -p farhelm-gui --lib` selected the exact
  `launcher::restart_with_tests::restart_with_an_idle_agent_asks_no_consent` case. It passed in run
  `fa0d73b9-5e27-49c9-83b2-7532c13ca4ed`. Enter applied Default, sent one Restart and left the permission to stop a
  working agent unset.
- The same nextest command selected `launcher::replace_with_tests::` and the exact
  `launcher::checkout_tests::the_guarded_answer_waits_for_the_new_preview_then_launches_once` case. All 19 passed in run
  `47c1099b-4976-47ef-8ae2-8ac85daf4478`, including both delayed answers, Cancel and the checkout hints.
- `python3 -B scripts/gui-e2e-smoke.py` passed 49 of 50 steps in run `62e7bdd1-6753-42ab-a12d-c5163b5cd7c4`. Headless
  Mutter skipped the clipboard step. Header Restart, row-menu Delete, Stop and Replace, and the YOLO answer ran against
  the desktop, helm and supervisor with scripted agents.
- `python3 -B scripts/check-gui-goldens.py` matched all 262 Linux captures to their reference images in run
  `8e4e29ce-3d80-4ad5-9553-77abe8ea6150`.
- `cargo clippy --locked -p farhelm-gui -p farhelm-grid-model -p farhelm-input-mapping --all-targets -- -D warnings`
  passed. So did `cargo fmt --all -- --check`, scoped dprint, `python3 releasing/check-changelog.py format` and the
  isolated `scripts/check-test-sleeps.py` check. The instruction-link checker reported two existing citation warnings
  and zero errors. The Wayland patch reverses cleanly on the vendored tree.

### Reused checks

- `cargo nextest run -p farhelm-gui --lib` selected the header click, Delete shortcut, newly available Replace, submenu,
  Clone and prompt presentation regressions on `d1a33808` plus the retained diff. All seven passed (run
  `e3031cdb-1bc5-4861-af4c-76e92fb8c1d7`). They exercised a click aimed at the old selection and a key dispatched before
  the replacement frame was shown. The header and presentation signal retained those behaviors on rebase.
- `cargo nextest run -p farhelm-gui -p farhelm --lib --test e2e` selected GUI menu, card and header tests plus
  `host_terminals::restart_of_a_host_agent_stops_its_tree_before_a_new_host_starts` on `c24b7166` plus the retained
  diff. All 92 passed (run `d39b96f2-877d-4c64-add0-fe69ae7d9da9`). The real supervisor refused the strict Delete guard
  after a session restarted, and the supervisor regression remains identical.
- `cargo nextest run -p farhelm-gui --lib` selected the environment, lifecycle, menu, header and card cases on
  `2c3ebab9` plus the retained diff. All 181 passed (run `5f8d11db-5a70-4206-bd77-01400f183536`). The exact
  `session_list::environment_tests::stop_and_suspend_keeps_its_verb_and_guards_across_a_push` case passed again from the
  same base after asserting that the source was ended and tabless (run `945aa18d-db3f-443c-a61d-f34dbd8ee1a2`). The
  environment code, test and resulting menu composition preserve those assumptions.
- The vendored Linux command was
  `cargo test --manifest-path vendor/gpui-pre-linux/Cargo.toml --no-default-features --features wayland,test-support --lib`,
  with Cargo path overrides to the vendored `gpui-pre` and `gpui-pre-wgpu` trees. On `655efe2d` plus the retained diff,
  all 14 tests selected by `linux::wayland::client::tests` passed (run `899f6803-116c-46aa-b822-4f4fdb7e8a6c`). Next,
  the exact `drop_uses_the_latest_motion` case passed on `2d789cef` plus the final patch (run
  `7b52033d-c566-4d64-90d7-8381362f24b1`). Both pointer motions preceded the file-list reply. The final patch and Linux
  backend are identical. Reapplying the patches to the published Linux crate reproduced the backend byte for byte.
- `cargo test --locked --manifest-path vendor/gpui-pre/Cargo.toml --features test-support --test native_key_metadata --test driver_record`
  passed on `655efe2d` plus the retained diff (run `694516bb-b0dc-4e8f-9280-bdcaf91754ff`). The rebase added main's
  independent hitbox-occlusion query, leaving native keys and the driver record intact.
- The separate `cargo test --locked --manifest-path` checks for `vendor/vte/Cargo.toml` with `--features ansi`,
  `vendor/alacritty_terminal/Cargo.toml` with `--lib`, and `vendor/pair-tests/Cargo.toml` each passed on `295e7da1`
  (runs `96351d68-06e2-4627-b23e-dee55a148904`, `5a94daf1-5af4-43b8-888f-c82eaddf8b99` and
  `5a4cc572-6ef5-4e75-85bb-b8d5de124ace`). The rebase brought no changes to those source trees.

The earlier `cargo nextest run -p farhelm-gui --lib` launcher, menu, card and prompt selection failed overall on
`1474ba21` plus the retained launcher diff (run `503ee299-6307-4d77-8e17-3036c1d875b0`). The menu, card, prompt, both
delayed Replace with answers and Cancel passed individually. The single failure was the new keyboard fixture's focus
assertion. The final exact rerun confirms that the fixture exposes Default and Enter submits it.

### Skipped checks

A native Wayland drag and macOS GUI runs were outside this local Linux run. The backend test drove the Wayland event
ordering, while the smoke covered desktop actions with scripted agents. The full workspace battery was skipped because
the targeted checks cover the changed paths and the desktop smoke covers their integration.

## Review

Each draft passed its required independent source review. Review tightened the live availability check for the
interrupted card and fixed the launcher Enter behavior. For the launcher's live warning, the shared prompt tests
exercise frame admission and the launcher cases check the displayed permission across both delayed paths and Cancel. The
additional launcher-specific raw-input test proposed during review was left out under the scoped allowance.

### Landing

#### What landed with it

It landed in a round of five plans: drawn-frame-actions, one-helm-per-supervisor, gui-ux-discards,
gui-ux-backend-test-gaps and gui-ux-gui-test-gaps. Since its base, main gained the previous rounds' plans (Shift+drag
marking, model family words, unusable-folder refusals and earlier ones) and bookkeeping. one-helm-per-supervisor, in
this round, uses the same signal the confirmation questions already use to wait until they are on screen; the two work
together. But this plan changed the shared test helpers so every simulated key or click first shows the frame, which
broke one-helm-per-supervisor's test of input that arrives before a question is shown; that test now sends its keys raw,
as this plan did for its own such tests.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The mechanism is about as
simple as the job allows: the app remembers what the last frame on screen showed, and a key or click acts only on what
was already on screen before it, so a command that first appears in the frame drawn for that very key does nothing. Two
PRs stayed under the allowances given through the two answered blocks (85 and 90 lines against 100). The launcher PR is
253 lines against about 240, slightly over; I accepted it, since the allowance was approximate and the extra goes to
covering both waits. The plan's fix to the Linux (Wayland) layer, so a dropped file lands where the pointer was
released, has no findings.

#### Fixes made while landing

- **Landing order.** The plan's stack has two PRs numbered 02, and the landing tool took them by name, putting the
  launcher PR before the session menu PR it builds on. After the first PR merged, the next conflicted; I put the
  launcher PR back on the session menu PR, confirmed each remaining PR's change was identical to what was reviewed and
  tested, and landed the four in their real order after the round's other plans.
- **SPEC_impl.md lost the rules this plan was meant to keep true.** The prompt section's "No new generic precondition
  protocol is added; new precondition checks go only where SPEC already requires them", and the sentence that
  confirmation questions exist to stop actions that ran on state the user was not shown, were deleted. Both are back.
  The section also now names the one exception to "a confirmation freezes what it shows": the launcher's YOLO question
  keeps its warning live.
- **SPEC.md now says a stale press is ignored.** It said a press uses what the frame the user was shown said; the code
  ignores a press whose frame was not yet shown or whose target is no longer in view. It now says so.

#### Things you should know that the report does not say

- **Two test gaps.** Nothing tests that the launcher's main Launch press uses the shown warning rather than the live
  one, which is the basis of this plan's first fix and its release note; and the New dialog's wait for a checkout
  preview before launching lost its only test that it does launch once the preview is ready, so its three remaining
  tests would pass if it never launched. Both are small follow-ups.
- **A busy session can swallow a click.** The snapshot compares the whole session row, its last-activity time included,
  so while a session is working, its row changes with each activity sample, and a click on that row or its menu just
  after a change can do nothing. I did not measure how often; confirmation questions make the same trade.
- Several comments were cut down (Stop's ask-or-send rule, which environment actions do nothing, a caution about when
  Clone may be opened). I left them; none affects behavior.

#### Checks

The five plans change the desktop app's GUI crate, the supervisor, the helm, the shared protocol, a patch to the
vendored GPUI Linux backend, the terminal emulator's tests, the brick-test harness's tests, the dependency lockfile and
the specs. The landing ran the lints and the dependency checks, the whole Rust workspace suite, the desktop process's
tests, the vendored GPUI core's tests, the brick-test harness's tests, the app's end-to-end smoke and the reference
screenshots (goldens). Builds ran locally; the workspace suite and the smoke ran on boat sandboxes (cloud VMs), from
binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, `python3 -B scripts/test-gui-e2e-smoke.py`,
  `cargo-deny --locked check licenses sources`, `python3 -B scripts/check-crate-layering.py`,
  `python3 -B scripts/dependency-notices.py --check --only-if-inputs-changed-since origin/main`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` once without and once with `--features agent`: passed.
- `dprint check`: failed on SPEC.md and SPEC_impl.md, which one-helm-per-supervisor left unformatted. Fixed.
- The brick-test harness's tests (`python3 -B -m unittest discover -s releasing/mac-vm-test`): 277 passed
  (`2476ddca-d615-4ebe-8be6-cfbd14160787`).
- The vendored GPUI core's tests: passed (`1309823f-52a9-4d34-8f5e-4490021331b7`).
- `cargo nextest run -p farhelm-desktop`, locally: 176 passed (`23619e63-3132-4115-a3de-1b8de2283a2d`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`1a1a1bf3-c564-4d63-bd5a-7f98ab136ff7`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed and the clipboard
  paste step was skipped, as always on Linux (`96f2b2cb-2797-4888-947c-a3d128988ac9`).
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats:
  8068 tests, 4 failed (`5ffcbf0c-a1d7-46e1-8857-b06fcd598a8d`, `495fea20-f8ed-4fa3-9344-67ff5337a9fc`,
  `8ad794ef-8b95-4032-b700-e1f244bd2965`):
  - the macro compile-contract test, which builds a small crate offline from this machine's cargo cache, which a boat
    does not have, so it fails there every time; it was therefore not run this round, and nothing in the round touches
    it;
  - the Add host take-over test, broken by stacking these plans (see one-helm-per-supervisor's notes). Fixed;
  - the real Podman environment scenario, which ran past its 30-minute bound while sharing its boat with the rest of its
    shard;
  - a test that a scoped Stop leaves alone the jobs of a stock tmux server a session started, whose fixture found the
    pane job in the session's scope.

  The last two were re-run alone on boats on the same code and passed: the tmux test
  (`feaf27df-b63c-4a0b-acb6-8afb442eace0`) and the Podman scenario in about 11 minutes
  (`2ec35bdd-4a33-4e4d-9cff-07018e16c5d5`). Both failures look like load on the shared boat, not this round's code;
  their runs are kept. This is one observation of each, so neither is recorded as a flake.

On the fixed tree (one test's early keys, the specs' wording and formatting, and comments):

- `cargo fmt --all -- --check`, `dprint check`, `python3 scripts/check-agents-md.py` and
  `cargo clippy --locked -p farhelm-gui -p farhelm-proto -p farhelm --all-targets -- -D warnings`: passed.
- The Add host test module, locally: 40 passed, the take-over test included (`1db5cf13-ab3f-414c-9678-4e806893dd50`).

The rest was reused for the fixed tree: after it only that one test, the specs and comments changed.

Skipped:

- The Mac-only checks: no Mac-specific code changed.
