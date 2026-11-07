## What this was about

Permanently deleting managed checkouts from the trash made the user wait for every file to be removed and disabled other
page actions. The maintainer chose immediate acceptance with background removal, a bulk confirmation that closes the
dialog, and progress on the trash icon.

The host now verifies each selected checkout, moves it aside within the trash directory, and answers before removing its
files. One background worker frees space sequentially without holding the lock used by session launches, restarts and
Deletes. Bulk confirmation closes the dialog immediately and sends one batch per host. An entry's own Delete keeps the
dialog open. The trash icon animates while requests or reported removal attempts are active, and its tooltip names the
hosts freeing space.

## Things you should know

Disk space returns after the trash entry disappears. Accepted removals survive a supervisor restart through the existing
working-copy records. A removal failure logs a warning, retains the hidden directory and record, and is retried only at
the next supervisor startup. Failed leftovers alone do not animate the icon or keep the page polling.

The existing identity checks, no-symlink/no-mount traversal and protection for retained session directories remain. New
sessions cannot enter either spelling of a directory accepted for removal, including through canonical aliases or a
fresh checkout rooted there. Review exposed a moved-ancestor alias case; the admission check now compares the root's
current location as well as its recorded location.

Refusals and uncertain replies remain available when the dialog is reopened. An unrelated successful delete does not
clear an uncertain result. Removing a host clears its notices, including notices from a late reply; a disconnection or
failed host refresh does not. The existing fifteen-minute request deadline remains for older hosts that still remove
files before answering. Older hosts omit the new progress indication and remain usable.

The Managed checkout trash section on the Session list documentation page now matches this behavior. Specifications and
the changelog were included with their corresponding code changes, so no separate documentation PR was needed. The
original slow-deletion TODO entry was removed. The live installation was not used.

## Open questions and possible follow-ups

No required work remains. The existing TODO about showing the user when trash cannot finish freeing space remains;
background failures currently have warning logs rather than a new user-facing notice.

## PRs

- [#3131: empty trash without waiting for file removal](https://github.com/scode/farhelm/pull/3131/changes) — draft,
  host behavior and recovery, commit `cd6be88bff9bd2a48d9d946ecbbcf39ff595942c`.
- [#3134: close trash after bulk cleanup confirmation](https://github.com/scode/farhelm/pull/3134/changes) — draft,
  dialog behavior, progress, retained notices and browser coverage, commit `46b3ec8e537e3efa0fbff00fc04192f879371d63`.

## Checks run, reused and skipped

Runtime checks used the repository recorder. Rust selections used pinned nextest 0.9.148, four slots, zero retries and
required pinned tmux. Selected-out tests are not passes.

- The final host safety selection passed 33 cases in `bccb24b1-88db-45d9-b0b7-b4af57ba9305`:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(checkout_trash) | test(service::trash_remover::) | test(existing_create_refuses_a_pending_archive_with_its_source_present)'`.
  It covers the reply before held removal, free host admission, startup recovery, retained failures, sequential
  attempts, identity refusals, journal collision rollback and direct, symlinked and moved-root launch refusals. It also
  verifies that startup recovery retains the state-directory claim after losing its awaiter. The recorded source
  fingerprint begins `10a18193294ffcb7`; the pushed host commit has the same tested behavior. Its complete JUnit report
  and retained output show no runtime skips, truncated output or forced cleanup. This final run supersedes the earlier
  32-case supervisor run `5b14a510-1fba-4094-8cf0-bc8bbf22f493`.
- Proto and helm trash tests passed in the earlier 32-case combined run `609cd896-c4f2-4939-b29c-6357f1764d35`:
  `cargo nextest run -p farhelm-supervisor -p farhelm-proto -p farhelm-helm --lib -E 'test(checkout_trash) | test(service::trash_remover::)'`.
  Only the unchanged proto and helm coverage is reused from that pre-correction run; the final supervisor selection
  above supersedes its host coverage.
- The UI trash selection passed ten cases in `0fd47051-71f8-4c69-b179-c75eda5d3d43`:
  `cargo nextest run -p farhelm-ui --lib -E 'test(list::trash::)'`. This includes frozen selections, confirmation
  authority and notice lifetime. The recorded source fingerprint begins `344858cc20324b20` and covers the final UI
  behavior.
- The CSS animation and token suites passed 34 cases in `9822576a-ac56-43fb-ad70-1dfc211abc6f`:
  `node --test app-css-animations.test.js app-css-tokens.test.js`, from the UI JavaScript test directory.
- The trash-dialog and GitHub-checkout browser specs passed all 78 cases, 39 per engine, in
  `ba46e9de-a6f8-4f69-96f6-f2528b3cbf98`:
  `npx playwright test tests/trash-dialog.spec.ts tests/github-checkouts.spec.ts`, from `e2e/`. Playwright 1.64.0 used
  three workers and zero retries on Chromium and WebKit. These cover immediate bulk closure, single-entry cleanup,
  motion preferences, retained refusals and uncertain results, removed hosts, targeted polling and real archive
  preservation. The complete report has no skips, interruptions, unstarted cases or global errors; all three stacks
  completed cleanup. The source fingerprint matches the UI unit run above. Chromium confirmation and WebKit
  preserved-trash screenshots were inspected.
- `cargo clippy -p farhelm-supervisor -p farhelm-proto -p farhelm-helm -p farhelm-ui --all-targets -- -D warnings`
  passed on the combined rebased source, including the startup ownership correction. Earlier per-unit lint runs also
  passed after test-style warnings were corrected. Browser prerequisite CLI and fixture builds and the Dioxus 0.7.10
  release web build passed before the rebase. Those artifacts were not treated as builds of the rebased tree. An initial
  invocation selected an unrelated executable named dx and failed before compilation; subsequent builds used the
  installed Dioxus CLI explicitly.
- Rust formatting, dprint for the changed files it includes, and changelog format passed. The isolated pinned-parser
  delay checker inspected 420 delays with no missing rationale. The documentation page is excluded from dprint and
  received a manual prose check; no headings or links changed.
- The full Rust and browser batteries, native desktop execution, installer, provisioning, release and CI suites were
  skipped: the focused host safety, protocol, UI and browser checks cover this change's concrete risks. WebKit browser
  coverage represents the desktop engine family, not a native macOS app check. No instruction, heading or path change
  required the agent-instructions checker. No docs preview was started during this unattended incidental docs edit.
- The stack was rebased cleanly onto `844a7209` after all six credentials PRs landed. Independent audits read the
  complete upstream changes and checked host admission, recovery, protocol forwarding, host-list authority, Settings
  ownership, shared API code and CSS. No semantic conflict was found. Both PR deltas were unchanged by the rebase before
  the separate startup ownership correction. The UI unit, CSS and browser results above are reused from their identified
  pre-rebase source: credential warnings do not change connected-host authority, Settings does not remount Trash, and no
  Trash algorithm, selector, modal or animation changed. The startup correction changes ownership retention only; fresh
  host tests cover it. No browser rebuild or rerun was needed for those preserved boundaries.

## Review gate

Each PR passed the required single fresh-context gpt-6.1-sol review at high effort, with the full test-authoring
checklist supplied. The host reviewer found two admission gaps, including canonical paths after an ancestor move; both
were corrected and the same reviewer cleared the final delta. A separate scope reassessment found no unnecessary
subsystem. A rebase audit also identified startup recovery's missing owner reference for in-process cancellation; the
correction and its real database-boundary regression passed the same host reviewer's recheck. The shipped process
already retained ownership through runtime shutdown. The UI reviewer found the uncertainty and removed-host notice
issues described above; both were corrected and cleared on recheck. Each PR's final text passed a valid fresh cold read.
A separate documentation pass covered every touched file.

### Landing

Landed with one guide-page sentence fixed. Deleting checkouts from the trash no longer waits for their files to be
removed: the host accepts the deletion, the entry leaves the trash at once, and the space is freed in the background.
Confirming **Delete all** closes the trash, and the trash icon shows while hosts are still freeing space; the trash no
longer shows each checkout's progress while it deletes.

- #3131: the host's background removal and the trash's immediate answer.
- #3134: the bulk confirmation that closes the trash, and the trash icon's progress.

#### What landed since the stack was based, and whether it interacted

Since this plan's base, main landed the session-row hover change (#3128) and moved checkout cache cleanup to the
background (#3130). A fresh-context review read this plan against both and against the other plan in this round,
stop-closes-tabs (Stop also closing a session's terminal tabs), which also changes the session list and how the
supervisor handles requests. It found no conflict: both this plan and the cache-cleanup change remove checkout files the
same way as before, and the stylesheet changes touch different rules. stop-closes-tabs was then sent back to its
executor because tests failed with it (see the checks below), and this plan landed alone; nothing of stop-closes-tabs is
in what landed.

#### Fixes made at landing

- The Session list guide page said a host retries removal "when its supervisor next starts" without saying what the
  supervisor is, which the docs' own rules require on first mention. It now says the supervisor is the program that
  keeps that host's sessions running and links to the page that explains it (in #3131).

#### Things you should know

- The scripts that capture the docs website's screenshots still stage the old trash: one expects the per-checkout
  progress and the progress area at the bottom of the dialog that this plan removed. They only run when the docs
  screenshots are captured, and that procedure starts by updating such scripts, so they are left to the next screenshot
  refresh.
- "host-settings scrollports stay reachable while saving" failed once on WebKit in this round's full browser run and
  once in an earlier landing's on the same day, each time passing on re-run. The landing monitor records it as a latent
  flake in a separate change right after this landing.

#### Checks

The round's checks first ran on this plan with stop-closes-tabs stacked on top, on main `ea1919b7`:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the lint of the UI's browser build,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`), `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three warnings about
  older text), the docs website build (`bun run build` in `website/`), the workspace debug build and the UI's release
  web build: all passed.
- The UI's JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`), run `a9fa4b38-e87f-492e-a1cd-07ef03eae671`:
  all 526 passed.
- Every Rust test in the workspace, `cargo nextest run --workspace --exclude farhelm-desktop` (with rootless Podman and
  its offline image present), run `1689f6c6-5913-4c2b-a109-28e876bc297c`: 5755 passed, 69 skipped by their own design, 3
  failed: two session-list re-render tests that stop-closes-tabs breaks (they passed on this plan alone, run
  `44345c89-c69e-4e73-8faa-582688471320`), and `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, a
  recorded flake with the same signature.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each ran its share of the suite's spec files through the test recorder,
  `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, built locally; the sandboxes run Node 24.16 rather
  than the development machine's 24.20), runs `9b11d18b-32eb-456d-9a23-46309281453a`,
  `0e536f3c-fa3d-4a52-8182-0596188a0e0e`, `d60b998e-a2fc-4e3f-89c1-757d55e7dcb3`,
  `6c5af047-1e2b-412a-bec1-9032a84e62b8`, `a67f189f-118a-48cc-b55c-ebb4449ece6a`,
  `db8e6273-26d9-4846-b576-6a2098bf8ac8`, `890c85a4-f445-43cf-84ba-5850e3f37994`,
  `e310a7df-b710-453a-b8d0-0e9649dac412`, `0570d0ff-8e46-433d-b636-c5901060350e`,
  `d653c87f-9fae-424e-84da-50fecea391cc`: 3028 passed, 14 skipped by the specs' own design, 12 failed. Three are
  recorded flakes with the same signatures (a settings save that waits for the browser's request-finished event, an
  update's progress label without a step name, and a quick-launch focus check); their spec files were not re-run. Eight
  passed when their spec files were run on this plan alone (below), so they come from stop-closes-tabs: two sidebar menu
  tests on both engines, a session-confirmations test whose Stop failed, and the three cleanups that session then broke
  in the terminal reconnect spec and the tooltip coverage spec (that one on both engines). The last, "host-settings
  scrollports stay reachable while saving" on WebKit, is in the modal layout spec and also passed in that run (see
  "Things you should know"). Then the suite's old-terminal set (the tests that only apply to sessions still in old
  terminals) on four sandboxes with the same build, runs `cdc1aa57-0488-4bc2-9279-8ec94051bc7b`,
  `ac26821e-8bee-46ef-bfe4-095f0dd483c6`, `d71f4950-3843-48cf-8148-7264ec7913ef`,
  `ae692651-eab1-4d16-b362-df250704a63c`: 107 passed, 1 skipped by its design.
- On this plan alone on the same main, after the guide-page fix: `dprint check`, the changelog format check, the docs
  website build, `cargo clippy --all-targets -- -D warnings`, the workspace debug build and the UI's release web build:
  all passed. The desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `0a6430b2-898c-430f-a037-ee9e53d6e089`: all 1091 passed. On eight Tensorlake sandboxes with a new build, the spec
  files for the sidebar, session confirmations, tooltip coverage, terminal reconnect, modal layout, the trash dialog,
  GitHub checkouts and the header, on Chromium and WebKit, runs `f7c0535b-f6f4-4657-92b4-c8a80231558e`,
  `0b4193c6-1fba-4396-9629-f1d37fe49c68`, `fd9017e0-8dc3-4326-87a9-d1dd0b402b4a`,
  `19e2ca02-9411-472f-8c6c-7d3ed7fbc9fa`, `459cecaf-e520-4b98-bd9c-00773beb6b4b`,
  `f20891c1-06ab-4540-b0c3-e046982ab376`, `80564c9a-3b61-4929-9cfc-89a28ab83935`,
  `2c518375-6a3f-4bab-9321-28b492f1dc5d`: all 454 passed.
- Reused for this plan alone, from the combined runs: the JavaScript tests, the rest of the workspace's Rust tests, and
  the rest of the full browser suite and of the old-terminal set. stop-closes-tabs sat on top of this plan, so those
  runs exercised all of this plan's code; every failure in them is accounted for above, and the specs where
  stop-closes-tabs changed behavior were re-run without it.
- Not run: the doctests (no doc examples changed), the grid terminal suite (no terminal code changed), the desktop smoke
  tests and the installer tests (no desktop shell, packaging or installer code changed), the docs website preview (the
  build above passed; no page was viewed), and the native Mac app (not runnable on Linux).

Nothing in the executor's report became untrue at landing.
