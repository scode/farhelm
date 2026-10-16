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

### Landing

Landed as #2078, #2085, #2092 and #2095, bottom-up, first of the round's nine plans; all nine were tested stacked on
main. Their only overlap was the review feedback index, where each PR removed its own entry; all removals were kept.

#### Review

An independent fresh-context review found nothing blocking. The drag check sees press, move and release before xterm's
link handler and gates every way a link activates, with the selection guard kept and desktop opening after every guard.
The stale-target recheck rereads every cell of the link, including wrapped rows, at hover and at click, within bounds
that fail closed. The clipping check runs on the live hover at click time, so a resize after hovering cannot bypass it,
and a hover that fits is never refused. Hidden-character escaping matches the shared unsafe list and covers every hover
line and outcome message. The four SPEC.md edits read consistently once stacked.

#### Fixes made while landing

- The clipped-hover refusal was raised above its hover (layer 41) and stayed there until the next click. Nothing
  isolates the terminal, so it could cover the session header's restart, replace and delete confirmations, which must
  never be covered, and it could cover a new hover whose click was then admitted. The refusal now drops to the ordinary
  failure layer, keeping its text, when a new hover starts or the pointer leaves; it stays raised over its own hover
  through a redraw, the case it was raised for. A JS unit test pins that, and the stylesheet's layer list says so.
- The browser test for that case expected the refusal to return to layer 41 after the quick switcher closed. Whether the
  dialog ends the hover differs by engine (Chromium ends it, WebKit keeps it), so the test now checks the actual rule:
  the refusal keeps its text and is never under a visible hover.
- The drag test re-hovered and pressed without waiting for the file lookup; an OSC file link shows a pointer before its
  lookup answers, so the negative could pass without the fix. It now waits for the ready disclosure, and checks that the
  release point is still on the same link.
- Changelog wording: dragging also no longer opens a file in the desktop app; "literal escape notation" is now an
  example of what such a path looks like.

#### Left as is

- A trailing space in a file name still hangs invisibly in the disclosure; a refusal because the link text changed is
  silent while the hover still looks valid; SPEC_impl.md does not yet record the drag gate and the click-time text
  recheck; no browser case drags a web link in the default mode.
- Suspected, not verified: a repaint that changes an OSC 8 link's target without its label keeps the old target (the one
  the hover disclosed); a hover hidden under the approval cards is still admitted; some Unicode lookalike spaces and
  right-to-left reordering in file names remain.

#### Checks

Run on all nine of this round's plans stacked on main (terminal-link-safety, ntfy-token-server-change,
remote-uninstall-fixes, update-failure-reporting, docs-accuracy-fixes, host-picker-dotted-i,
setup-advice-state-directory, copy-guidance-resize, brick-test-feedback-evidence), with every landing fix in:

- `cargo fmt --all -- --check`, `dprint check`, the changelog lint and the sleep checker (301 delays, none missing a
  reason); `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the UI crate's
  clippy with the desktop feature and `cargo check -p farhelm-desktop`; the brick-test tooling's own test.
- Through the recorder: the protocol and helm packages in full with pinned tmux, run
  `6352fad3-a3b1-48dc-9884-3efc17ee03c6`: 1258 passed; the CLI's own unit tests, run
  `0e1855d7-6f43-4da7-8a9a-495936ae565e`: 171 passed; the UI crate with the desktop feature, run
  `513addbb-2505-4c54-bc07-2ac2ec06713b`: 655 passed.
- The JS harness (257 passed) and the website build.
- The terminal-link-clicks, terminal-links, terminal-files, ntfy-settings, provisioning, settings and tooltip-coverage
  specs on Chromium and WebKit, run `97a26936-2b7b-42a5-af59-8f0fdb0b9d78`: 208 passed, 2 failed. Both failures were the
  clipped-disclosure test pinning the refusal's old layering after the quick switcher closed (see terminal-link-safety).
  After that expectation was fixed, the terminal-link-clicks spec on both engines, run
  `a698ea79-451d-4dd4-87f5-24ef1d4b1cad`: 16 passed. An intermediate run, `6113fd52-60bf-4730-ae85-5e9dc2e90d46`, failed
  on WebKit only because the first corrected expectation assumed Chromium's behavior.
- Not run: the supervisor package and the CLI's end-to-end targets, which no PR in this round changes; other browser
  specs.
