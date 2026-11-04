## What this was about

The website linked to Farhelm's Discord, but someone in the app looking for help had no direct route there. The plan
adds **Chat on Discord** immediately after **Send feedback** in the **?** menu, and the line "Rather talk in the open?
Chat on Discord" below the feedback dialog's summary. Both open the same invite in the browser, using the existing path
for Documentation.

## Things you should know

Following the dialog's link leaves the dialog and anything typed in it open. It stays in the header, before Close in
keyboard order, so Tab from Close still reaches Message. The menu retains its other items' order, with the desktop
update check last. Both links have hover text.

The updated docs page is `website/src/content/docs/docs/using/send-feedback.md` (`/docs/using/send-feedback/`). No
preview was started during unattended execution. Published release screenshots and their tests remain unchanged; only
the development visual-review helper's menu count and comment changed.

## Open questions and possible follow-ups

None required for this plan.

## The PRs

- [#2963](https://github.com/scode/farhelm/pull/2963/changes): Discord links in the help menu and feedback dialog, head
  `b905b8e0`.

## Checks run, reused and skipped

Run on Linux for this PR:

- `cargo nextest run -p farhelm-ui --lib`, through the recorder with pinned nextest, four slots and zero retries: 838
  passed, none skipped; run `c4baf5d9-d0fd-4802-acf1-6d98622f3d00`. This includes the menu order with and without the
  desktop updater. The only subsequent source correction was in a browser keyboard test; the Rust tested here is the
  committed Rust.
- The browser selection below, through the recorder with three workers, one temporary stack per worker, pinned tmux and
  zero retries: 40 passed, 20 in Chromium and 20 in WebKit; run `93d17e40-441e-44f2-9325-56110e00cf84`. It covers both
  new opener calls, retained draft text, menu order and keyboard navigation, the menu at 240px window height, feedback
  layout, tooltips, and keyboard scrolling including the sent dialog at 360x220.
- The host-terminal browser selection below, with `FARHELM_E2E_SUITE_TERMINALS=host` explicitly retained by the
  recorder: four passed, two per engine; run `c3aa3cf0-dfe1-4549-98c8-d2877251035d`. The suite checks that its temporary
  stack actually uses host terminals.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo fmt --all -- --check`, changed Markdown `dprint check`, `python3 releasing/check-changelog.py format`, and the
  isolated `python -B scripts/check-test-sleeps.py`: passed. The delay checker inspected 397 calls with none missing a
  rationale. This repository has no TypeScript or CSS dprint plugin; no formatter coverage is claimed for those files.
- `bun run build` in `website`: 35 pages built, all internal links valid. The release web build and matching
  `cargo build` also passed. The first web-build invocation resolved an unrelated command named `dx` and stopped before
  compilation; using the installed Dioxus CLI produced the successful build.

Default-terminal browser child selection, from `e2e`:

```sh
npx playwright test 'feedback\.spec\.ts|tooltip-coverage\.spec\.ts|filters\.spec\.ts|keyboard-scroll\.spec\.ts|modal-layout\.spec\.ts' -g 'feedback|help menu|rail Help menu|sidebar surfaces give every control hover text|sidebar menu flyouts|opens its page'
```

Host-terminal child selection, also from `e2e`:

```sh
npx playwright test 'keyboard-scroll\.spec\.ts' -g feedback
```

All three runtime records have complete source and runner reports, complete retained output under the recorder's 8 MiB
output cap, no runtime `SKIPPED` markers and no forced cleanup. No runtime test failed. The browser runs cover the
committed product and test files. The narrow and short-height screenshots were also inspected: the public link sits
below the private summary and the actions remain reachable.

No earlier-session checks were reused. Main changes through `2e2d60d1` were inspected: a separate terminal-hyperlink
fix, TODO edits and planning bookkeeping. They change neither the shared external opener nor help/feedback layout or
keyboard behavior, so no rebase or additional runtime run was needed.

The whole Rust workspace, full browser suite and unrelated cases in the selected files were skipped because these
focused checks cover the changed behavior. The staged-fleet visual-review run was skipped: only its count and comment
changed, and the same real web menu count is checked above. Native desktop execution and macOS were not tested; the
desktop feature compiles, and both new links reuse the unchanged system-browser path. No published screenshots were
refreshed. The instruction checker was unnecessary: no instruction file, Markdown heading or file path changed.

## Review gate outcome

The required fresh-context gpt-6.1-sol high reviewer found one affected keyboard test whose old focus origin no longer
preceded the scroll region after adding the enabled Discord link. The test now starts from that link, preserving its
pending-send scrolling and refusal-recovery assertions. The same reviewer checked the correction and reported no
outstanding findings. The focused browser runs passed afterwards. Commit and PR wording received a separate
fresh-context cold read, with no contradicted claim or convention issue.
