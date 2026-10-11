## What this was about

Three small bugs affected launching sessions and saving terminal output. After a successful launch whose reply was lost,
pressing Enter on an already-selected launch choice could start a second session. Failed template reads looked like an
empty list or a deleted template. Selecting text in a terminal kept as a frozen snapshot after reconnection failed no
longer copied it. You chose to fix these together, provided each stayed straightforward.

## Things you should know

All three fixes stayed within their complexity limits. Reapplying an unchanged effort, permission, trust, harness or
launch tab keeps the retry key; real changes still retire it and its associated YOLO question. The lost-reply browser
test starts a real session before dropping the reply, then proves the Enter retry uses the same key and leaves exactly
one session.

The launcher's `tl:` search now explains a template-read failure and offers Retry without discarding the draft. Opening
the launcher on a named template reports the read failure instead of claiming deletion. Other launch choices remain
usable. Clicking Retry follows the existing form behavior of dismissing search results; typing in search opens them
again with the refreshed catalog.

Frozen snapshots regain only selection-copy handling, using the live terminal's existing mouse-tracking and forcing
rules. Discarding the snapshot removes that handling, and a deferred copy checks that the snapshot still owns the
terminal. Input, resize and connection callbacks stay absent. No vendored terminal files changed. Chromium checks
clipboard read-back; WebKit cannot grant clipboard-read permission, so it checks the actual text passed to clipboard
writing instead.

## Open questions and possible follow-ups

No maintainer decision is required, and no outcome was dropped. Runtime copy coverage exercises ordinary selection in a
snapshot restored after takeover, alongside stalled-recovery and snapshot-removal regressions.
Mouse-tracking/forced-selection policy and a deferred copy racing with snapshot removal were checked in source, without
additional runtime assertions for those specific boundaries.

## The PRs

- [#1908](https://github.com/scode/farhelm/pull/1908/changes) — one draft PR for the three fixes, their outcome records,
  feedback removal and changelog entry. Ready state and landing remain the monitor's job.

## Checks run, reused and skipped

- Passed: `cargo build`; `dx build --package farhelm-ui --platform web --release` with Dioxus 0.7.10;
  `cargo clippy --all-targets -- -D warnings`; `cargo fmt --all -- --check`; terminal JavaScript syntax checking;
  changelog format lint; and dprint on the changed outcome ledger and feedback index. The isolated delay-source checker
  inspected 287 delays with zero missing rationales. A separate documentation pass covered every touched source/test
  file. Dprint has no TypeScript plugin here, so no TypeScript formatter pass is claimed.
- Recorded launcher Enter, idempotency, templates and quick-switcher specs on both engines, one worker and zero retries:
  run `9596c655-fa96-4c48-bb33-f25962081e27` executed 96 cases, with 94 passes and two failures in the new template
  Retry test. Its expectation required search results that the preceding clicks had dismissed. Its simulated error also
  omitted the build header and caused an unrelated mismatch banner. Both failures and their artifacts remain retained.
- Corrected the search-open premise and preserved the real helm build header in both new template-error fixtures. Run
  `21d07193-85c3-49e8-9167-a85a4d84ddc6` selected only those two tests on both engines: four passes. These are
  same-session fixture corrections, not latent flakes. The first run's other 94 passing cases remain applicable.
- Run `577e0696-82a1-40f9-a092-82cbb1acf690` selected takeover snapshot copying, stalled recovery and departed snapshot
  burial on both engines: six passes. All three recorded runs have complete reports, with no skips, timeouts,
  interruptions, unstarted cases or global errors. The original failed run remains a failed observation.
- Reused the builds and runtime evidence after the final rebase onto `fc5c0d9a`. Main added only plan claims and a
  delivered report; the full intervening diff had no product, specification or test interaction. The only later Rust
  correction combined equivalent conditions into a let-chain for Clippy; no behavioral rebuild was needed. Final
  bookkeeping and PR links do not affect the tested behavior.
- Skipped Rust runtime suites because no Rust tests changed, the JavaScript unit harness because the shared copy policy
  did not change, and desktop asset validation because the asset set did not change. Full browser/workspace batteries
  and hosted CI would add no identified coverage needed beyond the selected checks.

## The review gate's outcome

The prescribed fresh gpt-6.1-sol/high source reviewer found one fixture-premise weakness: remembered workspace trust
could make a supposed change unchanged. Explicitly selecting and observing false before the first request resolved it;
the reviewer accepted that correction. A supplemental review accepted the template-fixture corrections and equivalent
let-chain, with no outstanding findings or unread delta. Production correctness, small scope, listener removal and
unchanged-choice key handling passed source review.

Fresh wording readers accepted the final commit and PR text after a plain-language rewrite. The required native resume
check passed. A separate native cold reader passed report clarity and public-repository hygiene before delivery. These
are requested model selections; exact native model attribution and usage were unavailable. Private galaxy-brain evidence
is retained under session UUID `a63f6b2c-45f7-4830-bf84-ea76de63472d`, with its location recorded in the plan working
log beside the checkouts.

### Landing

Landed on 2026-10-11 (UTC) as #1908. The rebase met the review queue index; both plans' removals were kept.

#### Review before merging

No findings. A retry cannot launch twice or reuse a key for a changed request: the key is reused only when the whole
launch selection equals the held one, and the new checks only stop an unchanged choice from discarding a key that would
match. The snapshot copy listeners are removed with the snapshot, and nothing touches the link guards from #1832 or the
steady path hover from #1879. The template error uses the safe peer-text rendering. Notes left as they are:

- The snapshot copy test covers plain selection in a snapshot restored after a takeover; forced selection, mouse
  reporting and the copy-versus-burial race were checked in source only.
- The changelog says "Retry button" where the button reads "retry"; release curation can fix it.

#### Checks

These two plans were landed together, bell-build-mismatch-fixes first with launcher-and-terminal-copy-fixes on top.

- Run now, on the two stacked: `cargo fmt --all -- --check`, `dprint check`, the changelog lint,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
  `cargo check -p farhelm-ui --features desktop`; the UI unit tests through the recorder (run `88bbf3a0`, 454 of 454);
  the UI JavaScript tests (238 of 238); and on Chromium and WebKit with one worker and no retries the notifications,
  launcher-enter, quick-switcher, templates, terminal-create-idempotency and terminal-reconnect specs (run `9222a97b`:
  161 passed, 1 failed, described in bell-build-mismatch-fixes' notes).
- After the test fix: the notifications spec (run `fcc7695f`, 16 passed) and the sidebar's read-mark, unread and
  notification tests (run `6e2ea146`, 22 passed), both engines.
- Between those runs and the merges, main changed only plan bookkeeping.

Nothing in the report above was made untrue by the landing.
