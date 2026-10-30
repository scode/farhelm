## What this was about

The Settings gear looked squashed because its outline was wider than tall. It now uses the approved round, eight-tooth
design with a centred hole. A regression test checks that its outline stays square and centred.

## Things you should know

The approved proportions were used unchanged. The rail gear remains 16px and the host-menu gear remains 14px; colour,
stroke, accessibility and all callers are unchanged. The selectable gear used as a host's identity mark is a different
icon and was left alone.

The plan called the Settings dialog's gear 16px, but existing CSS already enlarged it to 28px. That size was preserved.
Actual app captures were inspected in Chromium and WebKit, in dark and light appearances, at all three existing sizes;
the gear reads cleanly beside Help and in the host menu.

## Open questions and possible follow-ups

None.

## PRs

- [#2826: correct the squashed settings gear](https://github.com/scode/farhelm/pull/2826/changes), draft, based on main.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-ui --lib`: 824 passed, zero skipped, recorder `9e1bbc1c-0b50-4a1d-8289-5a479510eac5`.
  After a lint-only correction to coordinate pairing, the focused geometry test passed again in
  `922fc4e9-e42b-4c34-8164-8b820cb08577`; the other 823 were selected out. Their earlier pass remains applicable because
  their code and product behavior did not change.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings`: passed in `7e7e470b-6e4b-4459-9abd-ab6d109bef12`. Initial
  run `9077351d-77fd-4302-ad45-edbd0afa061a` rejected the test's slice-chunk API; the suggested fixed-size API resolved
  it.
- `cargo fmt --all -- --check`, changed-Markdown `dprint check`, changelog format lint and the isolated test-delay
  checker passed. The delay checker found 327 delays and zero missing rationales. An initial formatting wrap was fixed.
- Backend/fixture build `4abb8c18-4585-43ad-b513-4681c27d5d52` and web release build
  `78b213ea-ef55-4701-8fb2-0923fcad9f92` passed. Private Playwright capture `f3835412-528d-41b3-bee3-68aa5a3f2533`
  passed both engine cases, each covering both appearances and all three sizes, against PR head
  `3a7cfddbc6a041f16ec6502ccaea5fe210e6f360`. This used generic recorder mode with a private config; its two passes were
  checked in Playwright's report. All three owned stacks shut down cleanly.
- Earlier capture `15e7ca51-e621-470c-a32e-40844e382c61` stopped before tests because the private config resolved its
  stack script from the wrong directory. After correcting that, `88a4a4dc-bd0a-402d-9606-aa8c62e9e7e4` rejected the
  plan's inaccurate 16px header expectation. Correcting the capture to the existing 28px CSS produced the pass above.
  Both failures remain retained; neither required a product change.
- Runtime evidence was reused after rebasing onto main `3a68d3c57b35a601509fbbb0fcacde2d9069e79d`: incoming work only
  curated release notes and removed old fragments, leaving runtime inputs unchanged. Changelog lint was rerun. The full
  browser suite, desktop shell, installer and unrelated runtime suites were skipped because this changes only static
  inline geometry. No CI run was requested.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no issues and reaffirmed that conclusion
after the lint correction. The commit/PR wording also passed its fresh cold read. No findings were rejected.

### Landing

Landed as delivered, with no fixes at landing. The Settings gear is round again: centred in its 16-unit box with equal
width and height and eight evenly spaced teeth, where it used to be drawn wider than tall. A unit test measures the
outline so a redraw cannot bring the squashed shape back.

- #2826: the gear's new outline, its test, a changelog entry and removal of its TODO entry.

#### What landed since the stack was based, and whether it interacted

Only plan bookkeeping and shortcuts-overview (the keyboard shortcuts overview, landing right before this in the same
round, with its own test fix at landing) landed since; they share nothing but bookkeeping.

#### Things you should know

- The gear in the Settings dialog's header is drawn at 28px, not the 16px the plan assumed; the executor kept that size
  and looked at the gear at all three sizes it appears in.

#### Checks

This round landed shortcuts-overview and settings-gear-shape, stacked in that order on main `cb426c6a`. On that stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint, the binaries' lint,
  the desktop-build checks, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py` (three warnings, all about citations already on main), the docs website build
  (`bun run build` in `website/`), the workspace debug build, the UI's release web build and
  `scripts/check-desktop-assets.sh`: all passed. The check that every test delay states its reason
  (`python -B scripts/check-test-sleeps.py`) first stopped on its own time budget while the machine was heavily loaded,
  then passed when rerun alone.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `32aa6a58-e03d-48bc-bdc9-9f9d045c3e6b`: all 825 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `fb44ab43-fa4d-4466-a39a-9ca2b6a8e263`: all 1008 passed. The UI's JavaScript tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `db8e29b7-e456-4fee-bd94-756914f90cbd`: all 372 passed.
- `npx playwright test` in `e2e/`, the whole browser suite on Chromium and WebKit, three workers each with its own helm
  and supervisors, no retries, run `8f80714f-9f10-46f5-af67-be60e573be31`: 2818 passed, 14 skipped by design, 2 failed,
  both on WebKit and both in parts these plans do not change. "composer folder dropdown preserves keyboard launch and
  grouped history" is a recorded open flake. In the Send feedback dialog's test, "the dialog refuses blank and over-long
  messages without sending" failed on the disabled Send button's color, exactly as it did in an earlier full run on a
  stack without these plans (run `0a494db0-73b5-462c-8e8d-33510baf9df6`, 2026-10-29); the shortcuts overview edits the
  same spec file but only its Help menu test. Repeated five times alone on WebKit, runs
  `756ea4b1-29d9-4954-b8a0-31d9cae25dee` and `06d4ac29-871d-480f-9f75-6e1b2364758f`, each passed every time. Recording
  the Send feedback case as a latent flake is a follow-up to this landing.
- The shortcuts overview's visual-review count fix at landing went in after the browser suite had run. Nothing was rerun
  for it: the file is outside the suite, and the visual review needs passwordless SSH to the screenshot scenario's
  remote hosts.
- Reused: the whole Rust workspace run of 2026-10-30, `f0b300d3-7298-4a62-acbe-29bcb28a82ac`, on revision `b5cbad6f`
  (4555 passed, 41 skipped by design, and the known flakes "shallow pause resumes without reset or replay" and "a hung
  save releases waiters at their deadline" failed), for everything outside the UI crate. Since that revision main gained
  only UI-crate changes (#2816, #2817), browser tests (#2818), docs and bookkeeping, and both plans change Rust only in
  the UI crate, all covered above. Skipped: the desktop smoke tests (no desktop-specific code changed) and the native
  Mac app (not runnable on Linux).

The report's checks were the executor's selected runs; this landing ran the UI's tests and the whole browser suite with
both plans stacked. Nothing in the executor's report became untrue at landing.
