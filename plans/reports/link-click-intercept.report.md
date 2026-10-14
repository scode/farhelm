## What this was about

Clicking a web link in a terminal could open two browser tabs when a program such as Codex also handled the mouse click.
The maintainer chose to let Farhelm own recognized web-link clicks by default, with a shared setting for programs that
need another behavior. Filtering forwarded URLs would miss local macOS sessions, where Codex opens the browser directly.

The default now opens the link through Farhelm while withholding its primary-button press, release and drag from a
mouse-reporting program. Plain-text web links and OSC hyperlinks follow the same rule. Ordinary clicks, other buttons,
file downloads and program-initiated URL forwarding keep their existing behavior. Forced selection still works.

## Things you should know

Settings has a separate terminal-links section. The alternatives either open through Farhelm and pass the click to the
program, with a possible second tab, or pass it only to the program. The latter opens nothing when the program has no
mouse reporting. Existing installations get the default.

The choice covers every client of the helm. It changes all mounted terminals in the current client immediately, without
reopening them. Other open clients pick it up when they reload, matching the existing host choices.

Review found an overlapping-button case: another button could leave mouse reporting active and leak the intercepted
primary release or drag. The fix keeps ownership until that primary release while allowing the other button's reports
and normal cleanup. A drag starting on an intercepted link sends the program nothing and never shows program-copy
guidance; its press still dismisses an existing copy notice.

The fix, shared setting, documentation and changelog are one coherent draft PR. This avoids exposing a setting before
its behavior works. The covered TODO entry is removed. Nothing is marked ready or merged by the executor.

## Open questions and possible follow-ups

Once this ships, repeat the original Codex case on a remote host and on the Mac: one click on a link in Codex's answer
should open one tab. Browser validation covers Chromium and WebKit, the desktop renderer's engine family, but does not
exercise the native Mac app shell or a real Codex browser opener.

## PRs

- [#1962](https://github.com/scode/farhelm/pull/1962/changes) — terminal web-link clicks open once by default, with the
  helm-wide alternatives.

## Checks run, reused and skipped

- Focused Rust preference, migration and ownership checks passed 35 cases in run `61354065-287c-4372-ba87-06a4e472cee5`.
  A follow-up passed seven fixture-parser and remaining downgrade cases in `61cb823e-d6b8-4f0d-add1-429feeff6303`. These
  storage and parser paths did not change afterward.
- The terminal asset JavaScript harness passed 240 cases with zero skips in `c10040ae-a8e8-4021-94bf-f61b9bb5dc32`,
  including mouse-report filtering and unrelated-input preservation.
- The stationary-fixture overlapping-button regression passed all four cases with zero skips in
  `08319b05-85bc-4599-8e14-2d77d663e232`: both engines, both button orders, SGR drag and legacy mouse reporting, outside
  release, preserved secondary reports and a subsequent ordinary click.
- The corrected settings geometry and all terminal-link cases passed 12 cases with zero skips in
  `06581bd8-b498-4e44-848d-800667ec4b6e`. The later strengthened copy-notice lifecycle proof separately passed both
  engines in `caa93bf1-f0c7-4a41-8ce0-ec2ab7886970`, including dismissal of a real notice and silence beyond its grace.
- The remaining existing terminal-link, mouse-mode, settings and clipboard cases passed as a separate command: 65
  passed, nine skipped, no failures in `ec29dad2-e05c-4107-b4fd-3806c1a5c313`. All nine skips are WebKit clipboard
  cases: Playwright cannot grant that engine clipboard-read/write permissions. They are a coverage limitation, not
  passes. The new link and copy-notice cases ran on both engines without skips.
- `cargo fmt --all -- --check`, `git diff --check`, changed-file `dprint check`,
  `python3 releasing/check-changelog.py format` and the isolated `python -B scripts/check-test-sleeps.py` passed. The
  checker found 298 delays and no missing reasons. The website build and internal-link checks passed; its pages did not
  change afterward.
- `cargo clippy --all-targets -- -D warnings` and `cargo check -p farhelm-ui --features desktop` passed on the
  integrated change. Their coverage is reused for unchanged Rust application code; the later fixture-only output
  adjustment also passed focused all-target Clippy. CLI, fixture and web builds supplied the actual browser
  prerequisites.
- The initial browser overlap proof exposed the product leak on both engines in `31d61d70-0e51-491e-a203-2db248e66382`.
  The first correction exposed an unstable test pointer premise in `f8d3994e-44eb-4d36-b23e-703fb565f4ab`; the fixture
  now holds link rows stationary and verifies live text under the pointer. Both failed records are retained. An
  intermediate four-case pass preceded that final fixture strengthening.
- The related run `f94520cd-859a-498c-8654-353685c7430c` exposed checkbox-only geometry assumptions applied to the new
  radios and an existing terminal-renderer hover-cache assumption in the repeated-link fixture. Both were corrected in
  the tests; its failed record is retained, and its partial passes are not treated as a completed validation gate.
- No full Rust or browser battery, desktop runtime, installer, provisioning or release gate was run: the affected
  contracts have targeted evidence, and those other paths were unchanged. An ad hoc TypeScript check was unavailable
  because this repository does not install that compiler; no dependency was added and no typecheck pass is claimed.

## Review gate outcome

The required fresh-context gpt-6.1-sol reviewer at high effort reviewed correctness, design, idiomatic code and the full
test-authoring checklist. The overlapping-button leak and incorrect browser mouse-mode assertion were fixed. The
corrective review also caught acknowledgement output moving the test's hovered link; that fixture premise was fixed. The
final source gate passed with no remaining findings. A scope reassessment supported the narrow report filter and
rejected a microtask boundary that could expire between native listeners. Commit and PR wording passed a second fresh
cold read. A fresh scope review also strengthened the actual copy-notice boundary and hover failure diagnostics.
Implementation stayed local under the plan's no-workhorse requirement.
