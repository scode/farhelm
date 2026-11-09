# Per-host checkout folders

## What this was about

One checkout directory rarely suits every machine. Farhelm already supported a host's own directory, but configuring it
required a numeric host ID on the command line. Each host's settings now has a **Checkouts** section with a **Checkout
folder** field, between Appearance and Permissions.

## Things you should know

An empty host field uses the default shown in grey. **Save** sets the host's own directory; saving an empty field or
choosing **Use default** restores inheritance. The field needs an explicit save, and its footer says so. The global
field is now **Default checkout folder** in Settings → Checkouts; the launcher's inline setup still sets that default.

The default is read at launch, including for hosts already added. Changing either directory affects new checkouts;
existing ones stay where they are. Saving checks only the path's shape and does not contact the host, so disconnected
and suspended hosts can be configured. The host still checks whether the directory works when a checkout is launched.

The field is available for every host kind. Per-host post-clone commands remain command-line only and are not returned
to the browser. Agent host-edit commands and the host listing gain no new setting. This reuses the existing stored
override and launch resolution, with no new database layout.

The managed-checkout guide and the website's **Start a session** and **Manage hosts** pages describe the new choices.

## Open questions and possible follow-ups

Existing documentation screenshots of host settings and Settings → Checkouts are stale. They were deliberately not
recaptured as part of this plan. No product decision remains open.

## The PRs

- [#3250 — set checkout folders in host settings](https://github.com/scode/farhelm/pull/3250/changes), draft.

## Checks run, reused and skipped

- **Rust: 40 passed.** `cargo nextest run -p farhelm-helm -p farhelm-supervisor --lib`, filtered to checkout
  configuration, launch preview/missing-root and unusable-path cases, through the recorder. This covers the five new
  route tests, disconnected-host configuration, hook privacy, set/read/clear, unknown hosts, invalid paths, revision
  behavior and existing inheritance. Run `88bb21b8-0c42-4ccd-b975-2c0a448e12c1`, pinned nextest 0.9.148 and tmux 3.7c,
  four slots, zero retries. The 3,403 selected-out cases are not coverage.
- **Browser: 28 passed**, 14 Chromium and 14 WebKit. `npx playwright test` selected the host-folder, Settings,
  checkout-launcher, host modal, tooltip and host-alias cases through the recorder. Run
  `bf5fa261-23d4-4759-8780-f2c912743f62`, three workers with separate stacks, zero retries and zero skips. This includes
  real saved-setting readback and launch preview, both clear paths, inheritance placeholders, invalid-path refusal,
  narrow layout and the held host-save keyboard boundary. WebKit is Linux browser coverage, not a native Mac app run.
- Both recorder reports and output are complete, with no omitted or dropped output and no forced cleanup. The manual
  evidence summary covers two successful commands and 68 executed passing cases. No failed attempt occurred in this
  plan. Successful screenshot attachments were automatically pruned by the recorder; no manual screenshot review is
  claimed.
- `cargo clippy -p farhelm-helm -p farhelm-supervisor -p farhelm-ui --all-targets -- -D warnings`, the CLI build and the
  pinned dx 0.7.10 web build passed. Formatting, changed-document dprint checks, changelog format and whitespace checks
  passed. The isolated test-delay checker inspected 430 delays with no missing rationale.
- Rust evidence was captured at working-copy `e638e361`; the later corrections affect only the UI field's external
  disable behavior, browser cleanup and specification wording, leaving that command's Rust inputs unchanged. Browser
  evidence covers corrected `fd499cb7`. Both apply to the final draft: rebase onto `25d8a943` added only an unrelated
  TODO entry, and the feature diff was unchanged. No runtime repetition was warranted.
- Skipped the full Rust/browser batteries and CI because the selected checks cover the changed behavior. Rename-only
  modal-focus and host-panel navigation-only specs add no uncovered host-field behavior. Website changes are prose and
  existing links, with no component or heading changes, so no website runtime build was needed. No live installation,
  real cloud sandbox or native Mac app was used.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort is clear after rechecking the corrections. Review
fixed browser cleanup that could race a backend save and an outdated save-order sentence. A further finding,
independently confirmed by the same reviewer, fixed keyboard access to the host settings dialog's scrolling content
while another setting is saving. The field's own save keeps its existing focus-preserving behavior. The focused modal
tests pass on both engines. No finding was rejected and no requirement was weakened.
