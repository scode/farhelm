# Trash deletion progress

## What this was about

Permanently deleting checkouts from Trash could block the page for tens of seconds with little sign that anything was
happening. The maintainer chose to keep the dialog blocking and make its progress clear.

## Things you should know

Cleanup now proceeds one checkout at a time on each host, with hosts working in parallel. The current checkout shows a
spinner and elapsed seconds, queued checkouts say "waiting", and confirmed deletions disappear immediately while the
counts and size totals fall. Each host shows its completed count. Both trash cans turn upside down and bob; Reduce
Motion leaves them static and upside down. Close stays disabled, with hover help explaining the wait, and Escape stays
blocked.

Each checkout gets up to fifteen minutes to return its result. An individual checkout's reported issue stays visible
while its host continues. A lost or unconfirmed reply stops that host's remaining queue and warns that some folders may
already be deleted; other hosts continue. The listing is rechecked once the whole run finishes. Session Delete, which
moves a checkout into Trash, is unchanged.

## Open questions and possible follow-ups

None required. Progress is per checkout, so one large checkout can still hold the dialog while its elapsed time grows;
there is no file or byte progress and no cancellation, as agreed.

## PRs

[PR #2724](https://github.com/scode/farhelm/pull/2724/changes) — draft, `fix: show progress during trash deletion`.

## Checks run, reused and skipped

The recorded browser selection was
`npx playwright test 'trash-dialog\.spec\.ts|github-checkouts\.spec\.ts|popover-width-movement\.spec\.ts|modal-layout\.spec\.ts|sidebar-resize\.spec\.ts' -g 'trash|sidebar'`,
through `python3 scripts/record-test-run.py --runner playwright`, with pinned tmux, three worker slots and zero retries.
Run `d4aea187-1480-4552-8f07-285b8da8deb9` executed 54 cases: 42 passed and 12 failed, split equally across Chromium and
WebKit. Every failure came from two test assertions: comparing a saved rendered diagnostic with raw text, and expecting
pointer hits on disabled Close after hover had deliberately moved to its tooltip wrapper. Those assertions were
corrected without changing product code or weakening the separate disabled-control and visible-tooltip checks.

The narrow selection
`npx playwright test 'trash-dialog\.spec\.ts' -g 'one trash entry deletes immediately|trash deletion progress survives'`
then passed all 12 affected cases, six per engine, in recorder run `3b058a63-67d7-49bd-829a-8dd9b5fc6490`. The other 42
passing cases are reused because their product code, fixtures and assertions did not change. Both records have complete
source and runner evidence, untruncated output, no skipped cases or runtime skip markers, and clean process cleanup. The
initial failed record remains retained separately. This covers all 54 selected cases across the two runs, including real
archive preservation, concurrent hosts, failure isolation, visible hover help, reduced motion and an elapsed counter
advanced past the old sixty-second deadline. Wide and narrow progress screenshots were also inspected.

`node --test app-css-animations.test.js` passed all 17 CSS contract cases through recorder run
`ddff2b52-106e-464d-b8b1-5d06c258fec9`. `cargo clippy -p farhelm-ui --all-targets -- -D warnings`,
`cargo fmt --all -- --check`, `dprint check SPEC.md SPEC_impl.md TODO.md`, `git diff --check` and
`python3 releasing/check-changelog.py format` passed. The isolated interpreter's
`python -B scripts/check-test-sleeps.py` passed with 323 delays inspected and zero missing rationales; two earlier
attempts exceeded its internal time budget while builds were running. Backend and fixture builds and the final
`dx build --package farhelm-ui --platform web --release` passed before browser execution.

Runtime checks used the working tree based on `8bb18d7d`, with the narrow rerun covering the final assertion
corrections. Their coverage is reused for PR head `f8123310` after a careful rebase onto `37e7c8f6`: the complete
intervening diff changes planning records, unrelated TODO entries and an unrelated supervisor unit test's scope-manager
fixture, with flake documentation. The product and test patch is byte-for-byte unchanged by the rebase; TODO retains
both sides. No additional runtime run was warranted. Full Rust, native desktop, installer and release suites were
skipped because this changes the shared UI and its existing trash request's deadline, with no server or protocol change;
the focused browser and CSS checks cover the changed behavior. No live installation was used.

## Review gate

The required single fresh-context gpt-6.1-sol reviewer at high effort found a tooltip issue on native disabled controls.
The tooltip now belongs to a hoverable wrapper while Close stays disabled, and browser coverage asserts the visible
tooltip. The same reviewer checked that correction and the final test-only assertion corrections, reporting no
outstanding findings. No findings were rejected and no scope departure was needed. The commit and PR wording cold read
also matched the intended problem and approach.

### Landing

Landed as built. Permanently deleting checkouts from Trash could hold the page for tens of seconds with little sign of
progress. The dialog still holds the page, as you chose, but now deletes one checkout at a time per host, with hosts in
parallel, and shows the current checkout's elapsed time, which checkouts are waiting, each host's completed count and
upside-down bobbing trash cans; each checkout gets up to fifteen minutes. A checkout that runs out of time, or whose
reply is lost, stops that host's remaining queue with a warning that some folders may already be deleted; other hosts
carry on.

- #2724: show progress during trash deletion.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, main gained the previous landing round, which changed the UI code that sends requests to the
helm (reporting failed settings saves) and the shared stylesheet (the standard link style). A fresh-context reviewer and
I read this plan against both: the trash deletion uses its own longer request deadline and does not touch the settings
save paths, and its new styles do not overlap the link rules; its new animation changes at most eight times a second,
within the stylesheet's own limit, and is covered by the stylesheet tests. The reviewer found no blocker and nothing
that needs your decision.

#### Fixes made at landing

None.

#### Possible follow-ups

- The reviewer noted that each host's "N of M deleted" line counts, in M, entries Trash lists only as a problem report,
  with no checkout, while the header counts those separately; a host can read "1 of 3 deleted" with only two checkouts
  shown. The counts are consistent and match the wording SPEC.md asks for, so this is cosmetic; counting only checkouts
  in that line would avoid it.

#### Checks

This round landed five plans together, in this order: the Codex trust dialog's Quit variant (codex-trust-hint), emoji
widths in the terminal (emoji-streaming-corruption), Claude capture through resident wrappers (claude-wrapper-capture),
progress while Trash deletes (trash-delete-blocking) and a test pinning the tunnel listener's address-reuse option
(tunnel-bind-reuse-test). This round ran the following on all five stacked together on this round's main, before the one
comment fix made at landing (in claude-wrapper-capture):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint
  `cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason, `dprint check` on the changed Markdown files,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (two warnings, both about
  citations main already had), the docs website build, the workspace debug build, the UI's release web build and
  `scripts/check-desktop-assets.sh` (the desktop app bundles exactly the files the web UI asks for, now 28 with the new
  emoji width add-on): all passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, four tests at a time, no retries, run `b0c70ae7-5815-4e5d-bed3-20a73a84d43f`: all 4450 passed; 40 more were
  skipped by design (helper entries that only run as child processes of other tests, and tests that need a real
  signed-in Claude or Codex). The UI's desktop-only Rust tests, `cargo nextest run -p farhelm-ui --features desktop`,
  run `a58ba32c-0ad8-4f6c-835a-313a7052cd31`: 959 passed. The tests of the UI's own JavaScript and stylesheet, including
  its animation rules (`node --test` in `crates/farhelm-ui/js-tests`), run `8b354894-f313-4ca4-bb23-28ea92406be7`: 334
  passed.
- `npx playwright test` in `e2e/`, the whole browser suite on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `e1283879-7e6b-4fcd-99ab-c94dfb2b139d`: 2644 passed, 14 skipped by design (WebKit
  cannot be granted clipboard access), 2 failed, both on Chromium:
  - "expanded setup failure closes the menu below it" timed out waiting for the host actions panel to settle; alone it
    passed 3 of 3 (run `71120673-042e-49b4-8a82-db714b41f755`). No plan in this round touches the hosts panel; one
    failure under a heavily loaded machine is not recorded as a flake.
  - "inline checkout folder setup refreshes discovery and preview and respects host overrides" failed inside the test's
    own wait, which reads the body of every repository discovery response and hit one whose body the browser had already
    discarded. Alone on this stack it failed 1 of 3 (run `f76349e5-6e48-4ebf-b7c3-141e0573b0f7`) and then passed 15 of
    15 (run `49b4a7d3-6340-43dd-9873-b57b51776458`); on this round's main without the five plans it passed 15 of 15 (5
    in run `babb6cc9-3874-4c69-ad5f-d7effd7bf9a1`, 10 in run `a70b9775-910a-47b4-9128-e3f0b60d96ee`). No plan touches
    repository discovery or that test, and 2 failures in 19 runs against 0 in 15 is too few to tie it to the round. A
    possible follow-up, which belongs to no plan in this round and is listed here once for all five: have that wait
    check the response's address before reading its body, so an earlier response it does not need cannot fail the test.
- The desktop smoke test (`scripts/desktop-smoke.sh` with the pinned tmux and its own cargo target directory), run
  `9ba28070-489e-4b87-859f-db818a511d1e`: passed on WebKitGTK under Xvfb. The app's asset handler served every requested
  file, including the new add-on, and the relaunched app attached a terminal to its remembered session.
- After the landing fix, which changes only a test's comment: `cargo fmt -p farhelm-supervisor -- --check` passed.
- Reused: none. Skipped: the doctests (no documentation example changed), real Claude and Codex runs (the reports record
  what their executors observed), and the native Mac app (not runnable here).

The report's checks were the Trash dialog's browser tests and their neighbours (checkouts, popovers, dialog layout and
sidebar resizing); this round ran the whole browser suite and the stylesheet tests with the other four plans, and the
Trash dialog tests and the checkout tests this plan changed passed there (the checkout folder failure under Checks is in
a different spec file). Nothing in the executor's report became untrue at landing.
