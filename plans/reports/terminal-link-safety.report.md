### What this was about

Terminal file links use the hover's host, full path and size to explain what a click will transfer. Four defects could
undermine that choice: dragging could activate a link in a mouse-reporting program; partial redraws could retain an old
file target; a short window could hide required details; and invisible characters or collapsed spaces could make
different paths look alike. The maintainer chose four fixes, keeping the first three small.

### Things you should know

All four fixes stayed within the agreed scope. A shared gesture check rejects movement beyond four CSS pixels, including
a drag that returns to its starting point. File activation rechecks the current displayed text and printed token
boundaries. A clipped or off-screen disclosure refuses transfer and opening with a readable explanation, which stays
below an active modal. Path display now exposes unsafe characters and literal escape notation while preserving repeated
spaces. No terminal-library patch, new endpoint or persisted state was added.

The wrapped-redraw regression uses the plan's permitted unit-test fallback rather than a new repainting program. Browser
tests cover the shipped file-link integration. Native save/open admission is covered in the JS harness; this work did
not run the desktop shell or an actual native application open.

### Open questions and possible follow-ups

None blocks this plan. The terminal library's own partial-redraw invalidation remains an upstream matter. The separate
TODO about false mismatch warnings for wrapped web links is unchanged.

### The PRs

- [#2078](https://github.com/scode/farhelm/pull/2078/changes): reject drags across terminal links.
- [#2085](https://github.com/scode/farhelm/pull/2085/changes): reject file clicks whose displayed target changed.
- [#2092](https://github.com/scode/farhelm/pull/2092/changes): refuse transfers while the complete disclosure does not
  fit.
- [#2095](https://github.com/scode/farhelm/pull/2095/changes): distinguish hidden characters, escape notation and
  spaces.

Each PR is a draft in one linear stack, with its own changelog fragment and completed triage record.

### Checks run, reused and skipped

- `node --test` in the UI JS harness: 256 passed. This covers the shipped helpers and native/browser admission seams.
- Recorded Playwright selection for all terminal-file cases plus clipping and path differences: 24 passed, 12 per
  engine, no skipped, unstarted or flaky cases. Run `a599fa56-aaef-41c6-a464-34c6f2927932`, using the final built
  behavior from the four-PR stack before its bookkeeping rebase.
- Reused the 44 passing Chromium/WebKit gesture and link cases from run `1982b41b-3c27-4656-b40d-f951c134f638`, tested
  at `2ac1a6a49b99`. Later changes preserve gesture ownership; the final file-link selection covers its changed
  downstream admission and display. No skipped or flaky cases in that earlier pass.
- `cargo build --locked` and release web builds passed in the owned build sandbox. `cargo fmt --all -- --check`,
  changed-Markdown `dprint check`, changelog format lint, and the isolated test-sleep checker passed; 301 delays had
  documented reasons.
- Retained two early browser-launch setup failures and two clipping-test failures. The latter exposed missing resize
  readiness and an unavailable test probe; both are corrected in the original PR and covered by the final pass.
- The stack was carefully rebased over the trash warning and deliberate-setup specification changes. The shared feedback
  index needed mechanical conflict resolution preserving both sides. Product assets and tests are byte-for-byte
  unchanged by that rebase, so the successful evidence still applies. The later installation-identity display change was
  also reviewed: it uses a separate Rust surface and does not alter the shared unsafe-character policy.
- Skipped broader browser, Rust runtime, Clippy, desktop-shell, installer and release checks: executable changes are
  confined to terminal assets; the Rust edit is a doc-comment cross-reference. The targeted checks cover the changed
  behavior, and incoming main changes introduce no uncovered interaction with it.

### The review gate's outcome

Each PR passed the required independent gpt-6.1-sol high source gate and a separate commit/PR wording cold read. Review
found and corrected a token-extension case, viewport containment after resize, refusal-message coverage during redraw,
modal priority, and a browser-test readiness assumption. The required scope review and its focused supplements found the
final design small enough for the agreed gate. The final source reviews have no remaining findings.
