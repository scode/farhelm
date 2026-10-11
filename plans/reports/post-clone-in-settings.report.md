# Post-clone command in Settings

## What this was about

Managed checkouts can run a shell command after cloning and before the agent starts, but configuring it required a
terminal on the helm's machine. Settings now has the all-hosts command below the checkout folder, with a multi-line
monospace field and an explicit Save.

## Things you should know

The agreed browser-access change is implemented: any signed-in client can read and edit the all-hosts command. Per-host
folder and command overrides remain command-line only and never enter these configuration replies. A host with its own
command keeps it.

Opening Settings preserves all three stored states: unset, explicitly disabled by an empty command, and a command. The
first two display an empty box without writing anything. Saving an empty or all-whitespace field clears the all-hosts
command; otherwise spaces and newlines are preserved. Enter adds a newline. A command over the existing 16 KiB UTF-8
byte limit is refused without echoing its text, changing the saved value, or advancing the configuration revision; the
draft stays available to edit. Loading and saving lock the field. A submitted save completes even if Settings is
dismissed.

Changes affect future accepted checkouts only. Already accepted preparations and retries keep their frozen command. The
existing configuration revision, command execution, and retry behavior were retained. The specifications, both user
guides, and changelog fragment now describe Settings access, and the completed TODO entry is removed.

One PR keeps the route, field, and documented access rule together. The field follows the existing folder field's save
lifetime but uses its own component; their newline and whitespace rules differ, so no generic settings framework was
introduced. The folder's existing Save and Retry buttons also received missing hover text, as the plan authorized. The
new field includes all four browser correction-disabling attributes from the start.

## Open questions and possible follow-ups

None required to complete this plan. The existing Settings dialog scrolls vertically; captures show the new field and
its outcomes, while earlier or later controls may lie outside the visible scroll area. A broader dialog redesign was
outside this plan.

## The PRs

- [#1858](https://github.com/scode/farhelm/pull/1858/changes) — configure the post-clone command in Settings, draft head
  `5f87af5c4af517118a452df22e23bba2ca4140da` based on `6b7deac3`; not marked ready or merged by the executor.

## Checks run, reused and skipped

- `cargo fmt --all -- --check`, both required Clippy configurations (`cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm --bins -- -D warnings`), `cargo build`, and
  `dx build --package farhelm-ui --platform web --release` passed. These cover Rust formatting/lints and the CLI/web
  product builds needed for browser validation.
- Focused helm checkout-configuration tests through `scripts/record-test-run.py`, child
  `cargo nextest run -p farhelm-helm --lib -E 'test(checkout_config::)'`: 23 passed, 1,019 excluded by the filter, four
  slots and zero retries. Run `6fbbd468-9402-45bb-84ae-2a49e8cf1be4`; no runtime early-return skip messages. It covers
  command states, authenticated route grammar, global-only replies, cap refusal and revision publication, beside the
  existing folder/inheritance cases.
- Settings and tooltip browser specs through the strict Playwright recorder on Chromium and WebKit: 20 passed, 10 per
  engine, zero skips/failures/flaky/interrupted cases, one worker and zero retries. Run
  `80ef5928-9c80-4183-addb-5424e974089e`. It covers opening without normalization, persistence, refused drafts,
  multiline entry, loading locks, save after dismissal, and the surrounding Settings hover surface. Empty, saved-command
  at 1280/390 widths, and refused screenshots were inspected for both engines; the command/help/buttons/outcomes are
  readable without horizontal clipping.
- Changed Markdown `dprint check`, `python3 releasing/check-changelog.py format`, the isolated
  `python -B scripts/check-test-sleeps.py` (283 delays, zero missing reasons), and the website
  `bun install --frozen-lockfile && bun run build` passed. These cover documentation, the fragment, test delay rules,
  and the changed guide's links/build. Final Rust formatting, Markdown formatting, and changelog format checks also
  passed after rebase. dprint has no configured TypeScript coverage here; browser execution compiled and ran the changed
  spec. No website deployment was requested or performed.
- Runtime evidence was gathered against base `6b7cb2f2` plus the implementation. Rust fingerprint
  `e8ba5833f99a1846bc0ee5260a907b1baccc874704140b0b121ab123f08687e4` preceded only a protocol docstring correction and
  browser capture framing; browser fingerprint `b8792ee370b556ad307b06fa2967de6c6b6b621292ed9201bf2e66870c837d97`
  includes those edits. Careful upstream assessment through `6b7deac3` found only unrelated TODO/queue bookkeeping,
  release-test operator notes, and a separate host-dialogs plan. The rebased tree differs from the tested product/test
  tree only in unrelated TODO additions, so runtime results and matching builds were reused for head `5f87af5c`.
- No full Rust/browser battery, desktop runtime, installer/provisioning, or release validation was run: execution and
  frozen-launch behavior were unchanged, and the targeted routes, UI lifecycle, both browser engines, and selected
  builds cover the concrete regression risks. No hosted CI was triggered.

## The review gate's outcome

Implementation and investigation stayed with the executor under the plan's no-workhorse rule. The required fresh native
`gpt-6.1-sol` high source reviewer checked correctness, design, idiomatic code, the full test-authoring checklist,
override exclusion, refused-write invariants and state preservation. It found one obsolete protocol docstring implying
the command never reaches a browser; that comment was corrected and the reviewer verified the correction. No outstanding
source findings remain. This source review did not independently execute tests or inspect screenshots; those results and
inspections are the executor's evidence above.

Two fresh native `gpt-6.1-sol` medium wording readers reviewed the commit/PR text. The first identified inconsistent
command terminology; the rewritten text passed the second cold read. Required resume reconciliation and the delivery
report cold read use fresh native readers on the executing session's inherited model. Actual native model attribution
and usage counters were not exposed, so requested models are recorded without claiming measured usage. Private
galaxy-brain records remain in the session `d0e361cc-2cf7-49dd-a656-7f0f6a284fbf` under the configured state directory;
the plan's working log beside the checkouts records the exact location and private evidence paths.

### Landing

Landed on 2026-10-11 (UTC) as #1858. The rebase met a conflict with text-fields-no-spellcheck, which landed first: both
added hover help to the checkout-folder field's Retry and Save buttons. The landing kept this plan's wording.

#### Fixes made while landing

All in #1858, before it merged:

- The "start a session" docs page still said the launcher shows "no checkout root is configured" until a folder is set.
  That message no longer exists; the launcher offers a field to save the folder instead, and the page now says so.
- The new Settings route accepted a command containing a NUL character, which the command line could never store and no
  process can run: every later managed checkout would have cloned and then failed at the post-clone step. Saving one is
  now refused with a message that does not quote the command, through the route and the command line alike, and a new
  test covers it.
- Saving a blank command cleared it but left the spaces in the field beside "post-clone command saved". The field now
  shows it cleared.

#### Review before merging

No blockers. The review confirmed the trust boundary matches the amended specification: the new route is behind the same
sign-in and origin guard as every other setting, per-host command-line overrides are neither read nor written by the
browser, and a saved command reaches the host and runs exactly as a command-line one did. Notes left as they are:

- A command set from the command line before this release, when the specification promised the browser would never see
  it, now shows in Settings on every signed-in client. The changelog fragment could say so; release curation can add it.
- A 16 KiB command made mostly of unusual control characters can exceed the supervisor's create-request size once
  escaped; the supervisor then refuses the checkout visibly. The command line had the same exposure.
- No end-to-end test runs a command saved through Settings; it goes through the same store the command-line tests
  exercise.

#### Checks

These two plans were landed together, text-fields-no-spellcheck first with post-clone-in-settings on top.

- Run now, on the two stacked, after the fixes listed in post-clone-in-settings' notes: `cargo fmt --all -- --check`,
  `dprint check`, the changelog lint, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings` and `cargo check -p farhelm-ui --features desktop`; the supervisor,
  helm, UI, protocol and CLI unit tests with the GitHub-checkout end-to-end tests, through the recorder with pinned tmux
  3.7c, four slots and no retries (run `817f0fdc`, 2895 of 2895); the UI JavaScript tests (230 of 230); the docs website
  build; and on Chromium and WebKit with one worker and no retries the settings, tooltip-coverage, terminal,
  github-checkouts, github-checkout-composer, feedback, templates and quick-switcher specs (run `ccef1907`, 270 passed).
- Between that run and the merges, main changed only release-testing documentation.

Nothing in the report above was made untrue by the landing.
