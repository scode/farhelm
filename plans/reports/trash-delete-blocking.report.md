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
