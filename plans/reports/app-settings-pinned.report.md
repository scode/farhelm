# App settings: pinned header and close controls

## What this was about

The settings dialog opened from the sidebar's gear could grow beyond the window, leaving its close button below a long
scroll. The approved design keeps the header and footer visible, adds an × close control at the top, and groups the
settings into sections.

The dialog now follows that design using the existing host-dialog frame. Only the settings body scrolls. Hosts, managed
checkouts and sounds have section labels; the installed Mac app also groups its automatic-updates choice under updates.
Both close controls return focus to the gear. Existing switches, help text, explicit save buttons, saving behavior and
Escape handling remain intact.

## Things you should know

The updates section appears only when the desktop updater is active. It could not be shown in this checkout: its layout
and unchanged save/refusal handlers were reviewed in source, and the desktop renderer compiled successfully. No hook was
added to force it into the web build, where a test verifies that it is absent.

The dialog reuses the host dialogs' header, scrolling body, sections and footer. A caller-supplied gear tile is the only
new header capability; existing host dialogs keep their marks. The only new CSS sizes that gear inside this settings
dialog. The feedback dialog and per-host settings retain their existing styling.

## Open questions and possible follow-ups

On a Mac running an installed release app, inspect the updates section at a short window height. That visual check
remains for the maintainer; browser execution covered the web build on Chromium and WebKit. The ordinary app-settings
documentation screenshot can be refreshed in the next routine screenshot refresh; published release screenshots were
left untouched.

## The PRs

- [#1949 — pin app settings close controls](https://github.com/scode/farhelm/pull/1949/changes), draft, bookmark
  `plan/app-settings-pinned/01-pinned-settings`, head `b0fc539400d6f2f3a2b0113742939bdc62f48f4e`.

## Checks run, reused and skipped

Run for this change:

- `cargo build` and `dx build --package farhelm-ui --platform web --release` prepared the real CLI, fixtures and browser
  bundle.
- `cargo fmt --all -- --check`, `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, and
  `cargo check -p farhelm-ui --features desktop` passed. The focused Clippy selection covers the affected Rust targets.
- `dprint check SPEC_impl.md TODO.md e2e/tests/settings.spec.ts`, `python3 releasing/check-changelog.py format`, and the
  isolated interpreter's `python -B scripts/check-test-sleeps.py` passed. The delay checker inspected 295 calls, with
  zero missing reasons.
- Recorded narrow browser run `7bd815a6-9d67-4aff-b42a-a6fa870255aa`: the new pinned-layout and dismissal test passed on
  both engines. It establishes real overflow before scrolling, verifies unchanged header/footer geometry and real click
  targets at normal and narrow widths, checks both focus-return paths, section order and no updates section in the web
  build, and checks that a tall window needs no body scroll. Captured normal, short, narrow and scrolled views were
  inspected against the mockup.
- Recorded `npx playwright test settings.spec.ts sounds.spec.ts provisioning.spec.ts tooltip-coverage.spec.ts`, run
  `3f00cdd9-f5c1-4a3c-a019-827593224a9b`: 144 passed, 72 per engine, zero skipped, failed, flaky, interrupted or
  unstarted cases, and zero retries. The recorder confirmed a complete report and successful cleanup. Real local and SSH
  hosts connected; provisioning uses the suite's declared simulated backend.

Two earlier narrow attempts, `57f331a2-75ff-4375-8eba-20a1ea8568e6` and `d5f6d5e1-c04f-4fa6-bdc1-e42aa0f9e9ac`, failed
during browser launch before the product assertions ran. The build sandbox first looked in the wrong browser-cache home,
then lacked browser libraries. Their evidence remains retained privately; runtime validation moved to the compatible
official Playwright image and passed. These failures were not replaced by the later passes or recorded as latent flakes.

Reused at final head: the successful checks above cover the same behavior as `b0fc5394`, based on main `1f2c461f`. The
history-only commit and removal of unrelated RSX formatting do not invalidate that coverage; no new main changes
required a rebase.

Skipped: the full Rust runtime and browser batteries, workspace-wide Clippy, desktop runtime, installer/release gates
and website build. This change alters dialog markup and layout without changing those behaviors or website source;
focused compilation and browser execution cover the affected contracts. Installed-Mac visual validation of updates
remains the explicit limitation above.

## Review gate's outcome

The required fresh-context gpt-6.1-sol reviewer at high effort found no correctness, design, idiomaticity or
test-contract issue after inspecting the diff, plan, mockup, screenshots and verbatim test-authoring checklist. A
separate fresh cold reader accepted the commit and PR wording. The implementation was done by the executor; only
independent reviews were delegated. The eight touched files received a separate documentation pass. No PR was marked
ready or merged by the executor.
