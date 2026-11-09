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

### Landing

Landed together with scoped-stop-without-sweep (cheaper cleanup for sessions in systemd scopes), stacked above it. Main
had just gained stop-start-message (a Stop during an agent's startup is now reported as such), the update floor
(installations and hosts older than v0.23.0 cannot update in place), and steadier terminal sizing on session switch. A
fresh-context review found no interaction with any of them; this plan's only supervisor change is three error messages.

It did interact with the fixed-height Settings dialog, which landed two rounds earlier. That plan's WebKit test holds
the Checkouts section to fit inside the 780px dialog, with only about six pixels to spare, and this plan's longer help
text under the default checkout folder wrapped onto another line in WebKit and broke it.

#### Fixes made at landing

- The help text under **Default checkout folder** in Settings → Checkouts is shorter: "Used by hosts without their own
  folder, which you set in a host's settings. ~ means each host's home folder. A missing folder is created on first use.
  Changes affect new checkouts only. Save an empty field to clear the default." It keeps what the executor's text said,
  except that "existing checkouts stay where they are" is now implied by "new checkouts only" (#3250). Checkouts fits
  again on both engines.

#### Things you should know

- Checkouts now fits in 780px by a few pixels, so the next addition to that section will need the dialog's height or the
  section's layout revisited.
- Three small docs wording points from the review, left as possible follow-ups rather than fixed at landing:
  - `docs/github-checkouts.md` should say that only the post-clone command (the command run after each clone) is still
    set from the command line alone per host.
  - The two uninstall pages say "**Checkout folder**" without saying whether they mean a host's own folder or the
    default.
  - The managed-checkout guide's "There is no default root" now reads oddly beside the new **Default checkout folder**
    label.
- The docs screenshots of host settings and Settings → Checkouts are stale, as the executor's report says.

#### Checks

The two plans were checked together, stacked in queue order on main, on this development machine and, at the
maintainer's request, on Tensorlake sandboxes. The tree that merged is the one checked last (`80366b10`, `ef23035c`);
main after the merges equals it outside `plans/`.

On the stack before the landing fixes (`2cc53615`, `160f30e2`):

- Here: `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`, the test-sleep
  checker, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop` and the docs website build passed.
- Tensorlake, browser: `host-checkout-folder`, `github-checkout-composer`, `settings` and `spawn` on Chromium and WebKit
  over six sandboxes (recorder runs `15407382`, `88777727`, `07584a58`, `8a2508fb`, `bb673a4a`, `8c459a18`). One test
  failed: the WebKit Settings navigation test's check that the Checkouts section fits without scrolling (run `8c459a18`,
  retained). per-host-checkout-folder's landing fix to its help text addresses it; every other test passed.
- Tensorlake, Rust: every Rust test in the workspace except the desktop app's, from one archive over nine sandboxes
  (runs `67cc46ef`, `271aa0c5`, `cf6e2b79`, `5c0fcb65`, `8dfee46b`, `14d0ff68`, `b9891b30`, `79ba2bd9`, `385cde9f`),
  with two failures.
  - `attribute_compile_contract_matrix` compiles small crates with a nested `cargo`, which the sandboxes do not have.
    Neither plan touches its crate (`farhelm-testtrace-macros`); it passed here last round on a tree that differs from
    this one only outside that crate (run `05991d87`), so that pass is reused.
  - `create_idempotency::a_crash_before_the_outcome_commit_still_replays_the_same_session` (run `cf6e2b79`, retained)
    had Delete refuse because the sandbox's per-user systemd manager stopped answering its probes, which is the
    situation that refusal exists for. It passed five times in a row here (run `84880653`, in the recorder's plain
    command mode, since its nextest mode refuses repetition). It also passed on a rerun from the same archive on three
    sandboxes, three times each, together with the whole create-idempotency and session-lifecycle modules (runs
    `41f38188`, `1ad1e77f`, `3aa87de6`). In that rerun, a different test on one sandbox reported the same manager
    unusable and skipped. This is one observation of the sandbox's manager, not a flake of the test.
  - The sandboxes have no stock tmux, so the new stock-tmux test skipped there; it ran here (next item).
- Here, on the same stack: recorded `cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e` filtered to
  the scoped Stop test, the scoped login-shell Stop test, the stock-tmux test and stop-start-message's early-Stop tests
  (`pending_stop_*`): 9 passed, none skipped, run `e5d5c56f`. This machine has a stock systemd-linked tmux and a working
  user manager, so the stock-tmux test really ran.

After the landing fixes: `cargo fmt --all -- --check`, `dprint check`,
`cargo clippy -p farhelm-supervisor -p farhelm-ui -p farhelm --all-targets -- -D warnings`, a fresh build and release
web build, and on Tensorlake `settings` and `host-checkout-folder` on both engines: 74 passed, runs `e107e43d`,
`90807ce6`, `94c646d1` and `6d9da565`. The Rust tests were not rerun after the fixes: in the supervisor they changed
only comments and the text of one informational log line, which no test reads. The browser runs used the recorder's
plain command mode (its Playwright mode cannot pick one engine per file) under the sandbox's slightly older Node 24;
every sandbox reported its per-user manager and SSH to itself working.

Reused: only the compile-contract pass above. Everything else that matters was run on this round's stacked tree, which
includes stop-start-message, so the executors' own runs on their older bases are not relied on.

Skipped: the rest of the browser suite (the specs above cover the Settings, checkout and launch paths
per-host-checkout-folder changes; the cleanup change has no browser surface beyond what the Rust end-to-end suite
drives), the desktop smoke (no desktop-only code changed beyond what the desktop compile check covers) and hosted CI
(manual-only, not dispatched).
