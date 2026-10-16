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

### Landing

Landed as #2113, the first of the round's seven plans; all seven were tested stacked on main. Their only overlap was the
review feedback index, where each removed its own entry; all removals were kept.

#### Review

An independent fresh-context review found nothing blocking: the hidden text is built only from escaped peer values, full
rows do not repeat the visible repository, and no browser test pinned a managed row's accessible name. The mark itself
was already released, so a "fixed" changelog entry is right.

#### Fix made while landing

Compact rows already add a hidden copy of the session folder, and the mark's new text adds the checkout root and any
differing session folder, so a compact managed row read the same path twice (three times for a borrower). The compact
copy is now added only for rows without a managed checkout, with a comment saying why. The changelog fragment now says
"normal and compact" rather than "both list densities".

#### Left as is

The long trash sentence is now in every managed row's accessible name, which makes rows long to listen to; an accessible
description was the triage's suggestion, and hidden text was explicitly allowed. SPEC_impl.md still describes the
location and lifetime as tooltip text only.

#### Checks

Run on all seven of this round's plans stacked on main (managed-checkout-row-accessibility, download-long-name-copies,
save-template-ime-escape, helm-start-without-ca-bundle, stopped-session-launch-error, trash-totals-unchecked,
checkout-override-error-wording), with every landing fix in:

- `cargo fmt --all -- --check`, `dprint check`, the changelog lint and the sleep checker (301 delays, none missing a
  reason); `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the UI crate's
  clippy with the desktop feature and `cargo check -p farhelm-desktop`.
- Through the recorder, after rebuilding the CLI: the protocol, helm and supervisor packages in full with pinned tmux,
  run `4256456b-4340-4e64-86a5-85aa71206a7a`: 2395 passed; the UI crate with the desktop feature, run
  `5d0c7c3a-4d29-4a92-a8e2-a1eba6893786`: 656 passed.
- The JS harness (257 passed) and the website build.
- The github-checkouts, templates, trash-dialog, sidebar, provisioning and tooltip-coverage specs on Chromium and
  WebKit, run `7f4cef76-eea8-4b05-9a31-1260bb14498a`: 456 passed.
- Not run: the CLI's end-to-end targets and other browser specs; no PR in this round changes them.
