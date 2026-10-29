## What this was about

The control for opening a terminal looked too much like another terminal tab. It is now a square **+** after a thin
divider, with the accessible name **New terminal** and a tooltip explaining that it opens a shell in the session's
directory. Its action, neutral button styling and unavailable states are unchanged.

## Things you should know

The divider is decoration outside the button's click target. The brick-test instructions describe the new-terminal
button (+) after the tabs, so the same wording works with the previous labelled button and the new icon. The **Work in a
session** docs page identifies the control in prose and alt text. Its published tab-strip screenshot still shows the
previous control; this plan deliberately did not recapture or publish screenshots.

One existing WebKit toolbar-layout case has little time left under its 60-second deadline: it timed out twice, then
passed unchanged in 58.7 seconds. No geometry assertion failure was established. The timing cause remains unproven, and
neither the original selected browser run nor the reproduction batch was green. All selected cases have an actual
passing observation, with the failed runs retained separately. No test deadline or product behavior was changed to
obtain that pass.

## Open questions and possible follow-ups

Refresh the tab-strip docs screenshot in a separately requested screenshot refresh. The WebKit layout case's timing
margin may warrant investigation if it recurs; this run does not establish a product regression or a latent-flake cause.

## The PRs

- [#2805: distinguish the new-terminal button from tabs](https://github.com/scode/farhelm/pull/2805/changes), draft,
  head `477928c9aa94e08ce4f01bf9e63ea2fab937cb3a`.

## Checks run, reused and skipped

Run now:

- `cargo fmt --all -- --check`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, scoped `dprint check`,
  `git diff --check`, `python3 releasing/check-changelog.py format`, and `python3 scripts/check-agents-md.py` passed.
  The instruction checker had zero errors and three existing citation warnings. The isolated interpreter ran
  `python -B scripts/check-test-sleeps.py`: 322 delays inspected, zero unexplained.
- Recorder `cb9d2cc7-8bf2-4731-aabf-b8ec1a84ce7a`:
  `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v`, 214 portable coordinator tests passed.
  These use fictional fixtures, not actual VMs or release checks.
- Recorder `508e9e72-980c-4fbe-b837-4a52665e7b70`: `cd website && bun install --frozen-lockfile && bun run build`
  passed, 32 pages and all internal links valid, using Node 26.12.1 and Bun 1.4.2. The preceding two recorded commands
  tested the complete dirty change above main `bef4fc05ea1ec602fb97135264656f4fba043a75`, fingerprint
  `cb7b3521557e7a02481ef8a0df436a5ecffbdb45bb6591da6e3751755a440609`; those exact bytes became the PR head.
- `cargo build -p farhelm -p farhelm-fixtures` and Dioxus 0.7.10's
  `dx build --package farhelm-ui --platform web --release` prepared the browser stack. Two initial web-build invocations
  selected an unrelated `dx` executable and failed before compiling; selecting the Dioxus executable corrected the
  invocation.
- Recorder `19b344e5-b3b4-4f45-ae92-512a9d9fcc21`, Playwright mode, ran the following selection from `e2e`, with
  Chromium and WebKit, three workers, zero retries, Playwright 1.64.0 and pinned tmux 3.7c:

  ```sh
  npx playwright test 'tests/(buttons|header|tooltip-coverage|terminal-tabs)\.spec\.ts' -g 'normal, neutral|native tunnel preview|tabs and actions share|header actions stay ordered|the session view gives every|a tab runs a real shell|the strip labels tabs|repeated activations while an open|an open the supervisor refuses|closing a tab confirms'
  ```

  Result: Chromium 10 passed; WebKit nine passed and the native-tunnel toolbar geometry case timed out. No skips,
  unstarted cases or interrupted cases. The successful cases cover button tiers and the new accessible name, shared
  toolbar geometry and action order, tooltip coverage, and tab shell, labels, close, repeated-open and refusal behavior.
- Recorder `13f31fe8-cb80-4a0d-85f8-507cf557baf3` was a failed reproduction invocation: an anchored grep matched no
  tests. It is not passing evidence.
- Recorder `733f7113-5cfc-4c70-854d-193ced1a9c97`, generic repetition mode, ran only the failing case twice
  sequentially:

  ```sh
  npx playwright test --project=webkit-header -g 'native tunnel preview keeps ports and actions inside a 650px pane' --repeat-each=2 --workers=1
  ```

  First attempt timed out; second passed in 58.7 seconds at the unchanged 60-second deadline. The batch exited nonzero.
  Both browser selections used unchanged PR head `477928c9aa94e08ce4f01bf9e63ea2fab937cb3a`, fingerprint
  `a69ffd16370e233413066c96ae369e756d7946b195ae7a8840e0ce0ce90cf9c6`, on Linux x86_64 with Python 3.15.2 and Node
  26.12.1. Retained output and the failed trace were inspected. Records have complete source identity, stream EOF, zero
  output omissions and clean process-group cleanup; generic reproduction counts come from its retained runner output.
  Failed evidence and attachments remain private and retained.

Reused: the existing browser builds for the exact-case reproduction, because no source changed between runs. Main still
matched the PR's base at the final interaction assessment, so no rebase or repeated runtime checks were warranted.

Skipped: the full Rust and browser batteries, unrelated JavaScript tests, desktop/installer/provisioning runtime, and
actual Mac VM tests. This is a markup/CSS change with unchanged action and state logic; lint compilation and the focused
browser observations cover its implementation risks. Native app appearance and screenshot freshness are not claimed. No
preview server was started for this unattended incidental docs edit.

## Review gate outcome

The required fresh-context gpt-6.1-sol reviewer at high effort reviewed the complete PR adversarially against this plan,
including the full test-authoring checklist, and reported no actionable findings. A separate cold reader checked the
commit and PR wording. The documentation pass covered every touched file. The source TODO entry was removed, and the PR
remains a draft for the landing monitor.
