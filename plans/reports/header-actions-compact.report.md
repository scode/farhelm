## What this was about

Open tunnels made the controls at the top of a session cramped even in a wide desktop window. The compact style was
triggered by the tunnel indicator's presence. The controls now keep their normal labels and spacing whenever the row has
room, and compact only when the actual indicator, normal controls and readable tab area cannot fit.

## Things you should know

Resizing, changing tunnel text, hiding or removing the indicator, and interruption/recovery all update the measurement.
The decision uses normal-size controls, so the compact result cannot make the sizing decision alternate. Tunnel
placement and the existing menu and confirmation positioning remain intact.

Source inspection found no invisible stale-indicator path: an empty tunnel list renders none, and a failed read keeps
the previous visible list. Mapping indicators have a minimum width, and multiple tunnels display a nonempty count. The
original sighting was not reproduced; presence alone was sufficient to explain the unwanted wide-window style. Browser
coverage uses the established mirrored native tunnel indicator, rather than operating macOS tunnels. Both Chromium and
WebKit measured the real session controls, including their menus and destructive confirmations.

## Open questions and possible follow-ups

None required for this fix.

## The PRs

- [#3288: compact session controls only when needed](https://github.com/scode/farhelm/pull/3288/changes).

## Checks run, reused and skipped

The final tested tree is the tree pushed in `86ab2e7858359d8e68cd8d721e241caf2922870d`; submission changed only its
description and commit identity.

- Final browser selection, through the recorder with pinned tmux, three workers and zero retries:
  `npx playwright test '(header|restart-replace-progress)\.spec\.ts'`. Run `72462c01-a457-4d49-8cbf-ca78be3a478a`: **42
  passed, 21 per engine**, no skips or retries. It covers wide/floor metrics, content-dependent pressure, repeated
  resizing, stable paint samples, hidden/removed indicators, interruption and recreated tabs, menus, confirmations and
  lifecycle progress.
- The new interruption/recovery regression first passed alone on both engines: run
  `6fe44d5c-6048-4a5d-83a5-8ef2210c3941`, two passed. Before the review correction, the affected browser selection
  passed 40 cases in `2084f97f-a1af-4727-81ae-3cce88e62937`; this is supplementary evidence, not the final verdict.
- UI Rust tests, `cargo nextest run -p farhelm-ui`, through the recorder with four slots and zero retries: run
  `52892cd6-db73-463a-a04c-b5fdfd0a2a27`, **937 passed, zero skipped**. Reused after the correction because the Rust
  mount/drop wiring and native test behavior were unchanged; subsequent JavaScript and browser-test changes are covered
  by the final browser run. Its actual uncommitted draft identifier is source fingerprint
  `e8b9179e851a2f855ce16d50da318e90093d35f8976ac2d4ff5d32106b720084`, on base `1f19bb8c`; identity capture is complete.
- Passed `cargo build -p farhelm -p farhelm-fixtures`, the pinned release web build, UI Clippy with all targets and
  warnings denied, Rustfmt, JavaScript syntax checking, dprint for the changed Markdown, changelog format checking and
  the isolated-parser test-sleep check: 429 delays inspected, zero unexplained. Clippy was rerun successfully on the
  exact pushed revision `86ab2e7858359d8e68cd8d721e241caf2922870d`, so it needs no reuse assumption. The final web build
  includes the corrected JavaScript.
- All four recorded runtime commands have complete reports and output scans, no missing coverage records and no runtime
  skip markers. Broader workspace, full-browser, native-desktop runtime and cloud checks were skipped because the
  affected layout and lifecycle boundaries have direct coverage above. No new provider behavior is involved.

## Review gate's outcome

The required single fresh GPT-6.1-sol reviewer at high effort passed the corrected change. It found that a tab strip can
be absent initially or replaced after interruption; the fix now reads the current strip, and the new test verifies those
real component transitions under the same header. Its recheck found no remaining substantive defect. The commit and PR
wording also passed its independent cold read.
