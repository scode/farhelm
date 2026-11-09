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

### Landing

Landed alone in its round, as #3269. After the executor based it, main gained the best-effort mount check for
uninstalling a remote Mac, a fix to the macOS acceptance test of the desktop app's own uninstall, and TODO.md edits.
None of them touches the sidebar, Settings or automatic updates. The only conflict was textual, in TODO.md, where this
plan's entry and the Mac uninstall entry sat next to each other and both were removed. A fresh-context review checked
the change against main. It found nothing that had to change before merging, and confirmed the contracts the change
relies on. A newer choice for the setting, from either the prompt or Settings, clears the prompt's error. The error is
kept by the sidebar itself, which stays in place while Hosts is open, so a failure while Hosts is open still shows when
the prompt comes back. A session menu open below the prompt closes when the button or the error appears or goes away, as
it already did when the prompt's rows change. **Try again** saves only the preference and requests no host updates
again.

#### Fixes made at landing

- If the save's reply is lost after the helm has already stored the setting, Farhelm reads the setting again and finds
  it on. The prompt still said "Could not turn on automatic updates. Later updates will still wait for you." The error
  now shows only while the setting reads off, and SPEC.md says so. No test covers a lost reply; the change is one
  condition, checked by the build and the UI tests below.
- SPEC_impl.md still described the error of a failed preference save as belonging to the Settings dialog alone, and did
  not mention the new button. It now covers both.

#### Things you should know

From the review, left as they are:

- After a failed save, the error stays attached to the prompt. If the prompt goes away (every listed host updated) and
  later comes back because another host has an update, the old error comes back with it, as long as nothing newer was
  chosen for the setting. SPEC.md says the error survives opening and closing Hosts "while attention remains", which
  does not settle this case.
- The button's tooltip says it updates "the listed hosts" and that later updates install without asking. Turning the
  setting on also lets the helm update hosts the prompt is not listing, such as ones whose optional update was
  dismissed. The report's "things you should know" mentions this; the tooltip does not.
- Two checks in the new browser tests are weaker than they look. The check that the button disappears after a successful
  save would pass anyway, because the host rows turn into updating rows on the click. The **Try again** check passes as
  soon as the click lands, before the save is answered.

#### Checks

Run on the plan rebased onto main with the landing fixes, the tree that merged, on this development machine and, as the
maintainer asked for today's landings, on Tensorlake sandboxes:

- Here: `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`, the test-sleep
  checker, `scripts/check-desktop-assets.sh`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings` and `cargo check -p farhelm-ui --features desktop` passed.
- Here, recorded `cargo nextest run -p farhelm-ui --lib`: 924 passed, none skipped (run `4c2bc4c7`).
- Tensorlake, browser, using the release web build: the `host-updates`, `settings`, `host-rail-warning` and
  `bulk-session-actions` specs on Chromium and WebKit, one project per sandbox. 150 passed, none failed, skipped or
  retried. Runs `2454d459`, `463afb6b`, `890d42c8`, `0f668ecd`, `a4aeb22b`, `d911b783`, `8ea71195` and `481c5cd4`,
  recorded the generic way rather than in the recorder's Playwright mode, which cannot run one engine per sandbox, and
  under the sandbox image's Node 24, a slightly older release than this machine's. Every sandbox reported that the
  services the tests rely on (a per-user systemd and SSH to itself) worked. The `host-updates` spec holds the plan's new
  tests (the button, a failed save and its retry, and a session menu the error line moves); the others cover Settings'
  automatic-update switch, the rail's host warnings and the bulk session actions shown with the prompt.

Reused: nothing from the executor's runs.

Skipped: the rest of the browser suite, including the separate set of tests for old tmux terminals (the change is
confined to the host update prompt, its Settings preference and the menus beside it, which the specs above cover), the
workspace's other Rust tests (no helm, supervisor or protocol code changed), the desktop smoke (no desktop-only code
changed) and hosted CI (manual-only, not dispatched).
