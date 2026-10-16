### What this was about

In the default session list, managed-checkout rows showed the repository in place of the working directory. Screen
readers could not recover the directory or the explanation that the checkout moves to trash when its last session is
deleted: both were available only on hover. Compact rows also lacked the checkout-lifetime explanation.

The agreed small fix makes that existing explanation available as hidden text in the row's accessible name, alongside
the repository. Sessions working in a checkout subdirectory retain that distinct location too.

### Things you should know

Visible row content and layout are unchanged. Both list densities expose the checkout location, differing session
directory and checkout lifetime. The hover text is unchanged, and the shared mark also exposes the explanation in the
session header.

The change uses the existing clipped-text pattern rather than adding description IDs, props or a new row structure. It
stays within the recorded complexity gate. A fixed changelog fragment is included, and the resolved review finding is
removed from the pending review queue.

### Open questions and possible follow-ups

None required for this plan. Browser assertions verify the accessible names; no manual screen-reader or native desktop
test was performed.

### The PRs

- [#2113](https://github.com/scode/farhelm/pull/2113/changes): expose checkout locations and lifetime to screen readers,
  draft based on main.

### Checks run, reused and skipped

- Recorded
  `npx playwright test 'github-checkouts\.spec\.ts|tooltip-coverage\.spec\.ts' --grep 'borrowers retain|hover text'`:
  six passed, three each in Chromium and WebKit, no skips, failures or flaky results. Recorder
  `318ef6cf-bd1a-48b1-8b62-44bf9a4c5fcf`, one worker, zero retries, pinned tmux and complete report/console. The
  managed-checkout case covers original sessions and borrowers, including a differing subdirectory, in both densities;
  existing sidebar and session tooltip coverage also passes. The disposable test stack cleaned up normally.
- `cargo build -j 4`, pinned Dioxus web release build, `cargo clippy --all-targets -j 4 -- -D warnings`,
  `cargo fmt --all -- --check`, changed-Markdown `dprint check`, changelog format and the isolated
  `python -B scripts/check-test-sleeps.py` passed. The delay checker inspected 301 delays with none missing a reason.
- Reused the builds, Clippy, browser run, delay check and source review after careful rebase from base `3c5c4a61` onto
  `e83ad3ff`. The tested change was `289f3473`; its patch is unchanged after rebase. Read all incoming changes: Claude
  trust's numeric parsing, host-bound agent retries, template editing, Codex prose and plan tracking. None changes
  directory rendering, accessibility, the tooltip contract or the ordinary user-session launches exercised here.
  Formatting and changelog checks passed again after rebase; the later PR-link amendment changes bookkeeping only.
- Skipped a full Rust or browser suite, desktop, installer and release checks. Actual browser accessible-name and
  tooltip assertions directly cover this markup change; no Rust algorithm, service lifecycle, native desktop integration
  or installation behavior changed.

### The review gate's outcome

The required fresh-context external gpt-6.1-sol reviewer at high effort found no issues. Its complete findings were read
and checked against the diff, including the complexity gate and full test-authoring checklist. It performed static
review only; runtime validation was local. A separate gpt-6.1-sol medium wording cold read passed. The documentation
pass covered the changed component and browser helper. No unresolved finding or scope departure remains.
