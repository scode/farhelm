## What this was about

Updating a remote host and enabling future automatic updates previously meant using two separate places. The sidebar's
host update prompt now offers **Always update** whenever automatic updates are off and at least one listed host offers
Update. It requests those listed updates, then enables **Update remote hosts automatically**.

## Things you should know

The button works for one host and for mixed required and optional updates. If saving the preference fails, the updates
already requested still proceed. A red message explains that later updates will still wait; **Try again** retries only
the preference save. Opening Hosts and returning preserves that warning. A later choice in Settings supersedes an older
sidebar save result, and a successful opt-in appears as the enabled Settings switch.

This remains the existing helm-wide preference: enabling it can also start automatic updates for other eligible hosts
already waiting. The ordinary busy handling still settles a manual and automatic update that meet on the same host; this
change adds no update retry or ordering guarantee. The prompt's extra button and warning participate in the existing
session-menu positioning rules. The completed TODO entry is removed and an added-feature changelog fragment is included.

## Open questions and possible follow-ups

None required. No updater behavior or host eligibility changed, and no live installation was modified.

## The PRs

- [#3269 — enable automatic updates from the host prompt](https://github.com/scode/farhelm/pull/3269/changes), draft
  targeting main, head `9de00c8fec5a1dac815a1755b72e7413de52b19f`.

## Checks run, reused and skipped

Run now through the recorder:

- `cargo nextest run -p farhelm-ui --lib`, filtered to host attention and the relevant API test: recorder
  `ce3f74b2-b0a9-4de8-83d0-14e5fc9ad6d2`, 8 passed, 916 selected out. The first filter did not select the scalar
  preference module, so it provides no evidence for those cases.
- The same command filtered to the verified `api::preference_write_tests` scalar and receipt cases: recorder
  `1d99bc7c-522f-4717-9d11-2f1ce7b66e31`, 10 passed, 914 selected out. Together these cover host classification,
  geometry identity, field-local rollback, reconciliation and superseded writes. Both used pinned nextest 0.9.148, four
  slots and zero retries; no tmux was needed.
- `npx playwright test '(^|/)(host-updates|settings)\.spec\.ts$'`: recorder `64cc4b13-c55c-447e-bb82-4433267fc084`, 100
  passed, 50 each on Chromium and WebKit, zero skipped or unstarted. Three workers drove three owned stacks with pinned
  tmux. The six added scenarios cover shown targets, existing opt-in, refusal while Hosts hides the prompt, both
  recovery paths and a warning moving an already-open session menu.

All three commands completed with complete source captures and reports, stream EOF, no retained-output omissions or
console drops, and no forced cleanup. The retained output has no runtime `SKIPPED` markers. There were no failed runtime
attempts. The browser fixtures answer automatic-update preference writes locally, so their fake hosts cannot turn on
automatic updates in the test helm.

Also passed: UI Clippy with all targets and warnings denied, Rust formatting, changed-file dprint checks, whitespace,
changelog format and the isolated test-delay check (429 calls, zero missing reasons). The pinned Dioxus web build and
CLI build completed before browser validation. An initial command found an unrelated `dx` tool on PATH and failed before
compilation; the successful build used the repository's pinned Dioxus executable.

Reused after rebase: each runtime command, Clippy, both builds and the formatting/test-delay results cover the reviewed
working copy on `9801c06aba03a7ffd202a92bd0b23625cc977de4`. Incoming main changes through
`bfd331c200d522180b9d864bf5821a1c68aaddcf` changed only queue state and another plan's report. The feature interdiff was
empty, and the final commit's tree matches the rebased candidate. Changed-file dprint and whitespace checks passed again
after rebase; no executable or test input changed, so no runtime repetition was warranted.

Skipped: full workspace and browser batteries, native desktop/Mac, installer/provisioning suites and CI dispatch. The
identified risks are this shared UI preference writer, attention geometry and existing Settings behavior, covered above;
there is no backend, installer or native-platform change for those broader checks to exercise.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol/high reviewer found no correctness, design, idiomatic-code or
test-authoring issue. The commit and PR wording passed its separate cold read. Every touched file received a
documentation pass. No scope departure, omitted requirement or rejected code-review finding remains.
