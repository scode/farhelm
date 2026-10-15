### What this was about

Paths printed in a terminal already let users download a file from the session's host. The maintainer chose desktop
Cmd-click (Ctrl-click off macOS) to save and then open a copy with its default app, limited to `.md`, `.txt` and
`.html`. This also implements the chosen hover design: a file icon and name, host and full directory, decimal size and
action hints, with a size-tier stripe.

### Things you should know

Plain clicks still download everywhere. Browser modifier-clicks also only download. A desktop modifier-click saves
first; an unlisted type or an opening failure leaves the saved file intact and reports why it was not opened. The hover
advertises opening only when the desktop helm says that type can open. The shared policy uses the saved name's final
extension, ASCII case-insensitively.

As decided, saves have no quarantine flag, and HTML is allowed despite default browsers running local HTML scripts. Any
signed-in device of the desktop's own helm can request native opening, as it can already request a native save. The
normal server helm refuses opening before a transfer starts. Opening receives only the newly published local copy.

The unchanged `download-hover` documentation screenshot shows the old hover. Three existing findings stay queued; this
feature does not claim to repair them:

- [Mouse-reporting drags](../../review_feedback_queue/mouse-reporting-drag-activates-file-link.md): in programs that
  handle mouse input, dragging within an OSC hyperlink can activate it because no local selection exists. For a checked
  file this can download unintentionally; native Cmd-drag (Ctrl-drag off macOS) can now also save and open an allowed
  type. The queued finding is based on source inspection, without a mouse-reporting runtime reproduction.
- [Invisible path characters](../../review_feedback_queue/file-disclosure-misses-invisible-path-characters.md): the
  existing escape policy misses some invisible Unicode characters, so a disclosed filename can look like another one.
  The redesign retains that policy.
- [Oversized disclosures](../../review_feedback_queue/file-disclosure-cannot-scroll-oversized-path.md): long resolved
  paths can exceed the hover's visible area while download admission stays enabled. The new layout retains the overflow
  policy; it is not proof that the whole target and size are readable in that case.

### Open questions and possible follow-ups

The complete native gesture, download and default-app interaction was not driven end to end, and no macOS runtime check
was run. Separate tests cover the browser gesture, helm publication/opening boundary and Linux default-app dispatch. The
maintainer's installed app was not used. Screenshot refresh and the deferred link findings remain separate work; no new
product decision is needed for this plan.

### The PRs

- [#2034: open files from terminal paths in the desktop app](https://github.com/scode/farhelm/pull/2034/changes): one
  draft feature against main, at `7c999bfc`.

### Checks run, reused and skipped

- `cargo nextest run -p farhelm-helm --lib -E 'test(downloads::)'`: 11 passed, 1087 filtered; recorded run
  `2d04ec7c-bef9-4871-adcf-f61ea640217e`. Covers extension policy, server refusal before transfer, published-path
  opening, saved-success errors, stat hints and existing streaming/cancellation behavior. Pinned nextest and tmux, four
  available slots, zero retries, no runtime substrate skip.
- `cargo nextest run -p farhelm-ui --features desktop -E 'test(native_file_dispatch_does_not_wait)'`: one passed, 632
  filtered; run `148042b5-0e94-4164-81f0-14820b85d036`, pinned nextest, four slots and zero retries, no tmux needed. The
  private default app receives the exact saved path, remains responsive after dispatch returns, then releases.
- UI asset JS unit harness: 21 passed; run `eac8696c-7fb9-4e6b-a5f5-d6174fa0a6ba`. Tests size formatting, tiers, icons
  and native/browser modifier handling. These early JS and helm results cover their recorded working tree on `7110c5ae`
  and are reused: later edits changed native OS dispatch, its regression, dependency wiring and prose, leaving the
  tested JS and helm behavior intact.
- Complete terminal-files browser spec: 20 passed on Chromium and WebKit, one worker, zero retries, pinned tmux; run
  `51d7af89-5e4d-46e3-b0a0-2e79793fef4c`. Local and SSH hosts both connected. Covers hover redraw/layout, decimal
  boundaries and stripe colors, complete downloads, OSC paths, browser modifier requests and cancellation. The focused
  tier case also passed on both engines, run `c8910a8c-2cd9-434a-97ac-f173589b4eff`. Its hover capture was visually
  inspected. Stack cleanup completed.
- Targeted all-targets Clippy for the helm and UI with desktop enabled, plus
  `cargo clippy -p farhelm --bins -- -D warnings`, passed. Both desktop UI and shipped desktop binary compile checks
  passed. CLI/fixture and release web builds passed. `scripts/check-desktop-assets.sh` passed its negative comparator
  checks and matched all 26 assets.
- `cargo fmt --all -- --check`, `dprint check`, changelog format lint and the isolated sleep checker passed; 299 delays,
  zero missing rationales. The website's frozen install/build passed with 32 pages and all internal links valid.
- Two browser attempts failed before product assertions: run `4d0b78b7-d3b0-4f3c-8ddf-667c3cfeec71` could not find the
  mounted browser cache; run `d56d4ea3-3101-401d-a0e0-6be03561ba79` lacked runtime libraries and used a cached WebKit
  for another Ubuntu version. Both failed records are retained. Correcting the owned sandbox's cache, libraries and
  matching browser download produced the narrow and full passes above. Earlier build setup refusals (tool PATH, CLI
  selection and offline wasm tooling) are also retained; none is counted as a pass.
- No rebase was needed: upstream changes concerned unrelated sidebar/OMP browser assertions, release-test machinery and
  queue bookkeeping. No terminal-file runtime, API or specification dependency changed. Validation covers the feature's
  unchanged source tree at `7c999bfc`; no prior plan's runtime results were reused.
- Skipped full workspace/browser batteries and hosted CI because the affected endpoint, assets, native dispatch and both
  browser engines have targeted evidence. Native end-to-end and macOS runtime are the gaps described above. Installer,
  provisioning and release gates cover unrelated behavior. No live app, deployment or release was run.

### Review gate outcome

The required fresh-context gpt-6.1-sol reviewer at high effort reviewed correctness, design, idiomatic code and the full
test-authoring contract. It found that Linux `xdg-open` could wait for the opened app to exit, leaving a completed save
pending and delaying desktop shutdown. Linux now uses GIO's default-app dispatch, already present through GTK. A
controlled-application regression proves dispatch returns while the application stays alive. The follow-up source review
found no residual concrete issues; it ran no tests. macOS uses `open` without waiting for the app's lifetime.

The documentation pass covered every touched file. Commit and PR wording passed their separate cold read. One coherent
draft combines the hover and opening behavior because their capability and endpoint contracts are shared; the PR changes
no `plans/` files. The covered TODO entry is removed. SPEC.md records the maintainer decision and the implementation
spec and user docs describe the new behavior.

Implementation stayed in the main session under the no-workhorse requirement. Source review used gpt-6.1-sol high
through codex exec; wording used native gpt-6.1-sol medium readers. Review evidence is retained in the private
galaxy-brain session record. External per-turn counters are available; exact reported model attribution, native reader
usage and main-session usage are unavailable. The delivery report also receives its required native cold read.
