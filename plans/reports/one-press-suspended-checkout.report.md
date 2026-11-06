# One Launch press for a fresh checkout on a suspended host

## What this was about

Starting a fresh GitHub checkout on a suspended Tensorlake host used to resume the host, then require another Launch
press to review its new directory. The maintainer chose automatic allocation after the host wakes, with no directory
review. The session launcher now resumes and launches from one press, including quick-launch and number-key templates.

## Things you should know

Before Launch, the suspended-host checkout shows no directory path. It says the directory will be picked when the host
is ready, or named after the session when a name is supplied. The launch waits for a fresh connection and a current
checkout preview; a preview from before suspension cannot authorize it. A template also keeps its accepted launch when
the host's checkout settings finish loading after the shortcut was pressed.

Unnamed checkouts, template base names and untouched Clone names keep their existing allocation and numbering rules. A
user-entered or fixed name whose directory is taken stays refused. If another launch takes the checked directory, the
launcher reports the conflict and requires another click; it never automatically retries a refused create.

The incidental documentation changes are on the Start a session and Tensorlake sandboxes pages. No preview server was
started for this unattended run. The matching TODO entry was removed and a changed changelog fragment was added.

## Open questions and possible follow-ups

No known outstanding behavior questions or required follow-ups.

## PRs

- [#3073](https://github.com/scode/farhelm/pull/3073/changes) — one-press resumed checkout launches; draft, based on
  main, head d11009dc8719790698d426de7de2dadd4a76b503.

## Checks run, reused and skipped

- Passed the final recorded browser selection below: 32 cases, 16 Chromium and 16 WebKit, zero failures or skips, three
  worker stacks and no retries, with pinned tmux. Run f59530ec-fbe8-4ca5-8e0f-b8038e3196b0 covered the same source tree
  committed in #3073. The new lifecycle cases control resume, preview and create replies to verify dispatch; the
  connected-checkout cases perform real clones and directory collisions in owned local fixtures.

  ```sh
  npx playwright test tensorlake-lifecycle.spec.ts github-checkouts.spec.ts -g 'suspended fresh checkout uses one press|quick launch (waits for a managed preview|numbers a template base|shows preparation)|launching in an existing folder|successful resume stays honest|failed resume refuses|delayed create after resume'
  ```

- The earlier combined browser run 9dc64e88-1d08-45b8-b414-ebf8a11dae72 passed 30 cases and failed the two WebKit
  template cases. Loading checkout settings after the shortcut cancelled its accepted launch. An isolated run on a fresh
  configuration passed, then the tests were made to force that ordering: reproduction
  ac88412b-d68c-4d5c-a223-a132569da1c2 failed both cases before the correction, and aa089577-a95e-4506-b6b9-2cef2a34b56c
  passed both afterward. Failed evidence is retained; this was fixed in this run.
- Passed `cargo nextest run -p farhelm-ui --lib -E 'test(list::create_form::) | test(github_checkout::)'`: 117 selected
  tests, run d4239298-4072-4edf-8b9a-4a1ee4486cc9, on the working tree based on 8622676b with source fingerprint
  636213a37d34bc2ce0d47edcc3c3397e24eefe0f4676842a4240db93a03dfd0a. Reused for the unchanged launcher, naming and
  preview logic: the only later Rust change was the resumed-stamp configuration comparison and its assertions. That
  final correction passed `cargo nextest run -p farhelm-ui --lib -E 'test(/(guarded|resumed)_submit_stamp/)'`: two
  tests, run 310b7eaf-2f56-4a70-a3e4-28aa41e9b417. Both used the recorder, four slots, no retries and pinned tmux;
  selected-out tests are excluded from these pass counts, and no selected test reported a runtime substrate skip.
- Passed `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the
  application build and the Dioxus 0.7.10 release web build.
- Passed Rustfmt, dprint on the changed Markdown, the test-delay checker (413 delays, zero missing explanations), and
  `python3 releasing/check-changelog.py format`.
- Skipped the full test battery, hosted CI, native desktop runtime and real Tensorlake calls: this changes launcher
  logic, covered by focused unit/browser cases and desktop compilation. WebKit checks the engine family, not a native
  macOS window. The docs edits change prose only, so no website build was needed.

## Review gate outcome

The single required fresh-context gpt-6.1-sol/high reviewer found one issue: Enter on a recent GitHub setup could submit
before the launcher rerendered. That was corrected and covered by a browser case. The same reviewer accepted that
correction and the later settings-discovery correction with no remaining findings. A separate own-model scope assessment
confirmed that the latter fits allocation after resume without weakening current-preview validation. Commit and PR
wording passed the cold-reader process. Main's subsequent change only blocked another plan; it changes no product code
or shared behavior, so no rebase or additional runtime validation was needed.
