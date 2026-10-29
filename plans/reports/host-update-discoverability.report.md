## Since the last review

The same two draft PRs remain. #2807 was rebased without changing its diff. #2808 now removes the automatic-update
preference from the shared fixture that reconstructs a version-26 database, gives the new Settings switch hover text,
and corrects the Update guide and Manage hosts page. The frozen historical database definitions and production migration
are unchanged. These address the latest Decisions following the unsuccessful landing round.

## What this was about

Remote-host updates were easy to miss in the session list. The Hosts button now shows an upgrade arrow and the number of
hosts needing updates. Optional updates get a dismissible card; required updates get a banner explaining why those hosts
cannot be used. Both update exactly the hosts they name or open Hosts.

**Update remote hosts automatically** in Settings under **Hosts** starts off. Enabling it lets the helm perform those
same authorized updates even with no window open. The choice survives helm restarts and is shared across clients.

## Things you should know

The existing Hosts panel's **Update all** retains its broader meaning, including eligible hosts already on the current
build. The reminders and automatic setting act only on outdated hosts. Automatic updates exclude the helm's own machine,
newer builds and busy hosts; busy hosts are reconsidered when their work finishes. The app's own automatic update
setting is separate.

Dismissing the optional card leaves the Hosts arrow visible. The card returns for a newly outdated host, a changed helm
build or an interface restart. Required updates take precedence, cannot be dismissed and hide the optional card. Both
reminders hide while Hosts is open.

A failed or refused automatic attempt waits for that host's next connection or a helm restart. A repeatedly failing host
can therefore be retried on every reconnect. Toggling the setting does not clear failure memory. Planning refusals are
logged; manual Update still shows the reason. Accepted runs keep ordinary progress and failure in Hosts. Turning the
setting off prevents new starts and leaves accepted work alone.

The settings database advances to version 68. This is a one-way upgrade: earlier helms, including the v0.32.0 release
candidates, cannot open it afterwards. No supervisor schema changes. The existing unreleased changelog fragment covers
both PRs, and #2808 removes both source TODO entries.

## Open questions and possible follow-ups

No outstanding product decisions or required follow-ups.

## The PRs

- [#2807 — update reminders in the session list](https://github.com/scode/farhelm/pull/2807/changes), draft, head
  `8f3d818a98c01d628ada04652f9b1f9b5d34c877`.
- [#2808 — automatic remote-host updates](https://github.com/scode/farhelm/pull/2808/changes), draft, head
  `477114bf1234e8e2c4306fd6f1eacb4a819dc88d`.

## Checks run, reused and skipped

Current round, on the combined stack rebased onto main with the four-file correction:

- `cargo nextest run -p farhelm-helm`, recorder `44d635f5-959b-4b2a-8f7f-da895ce653df`: 1,327 passed, none skipped.
  Includes both corrected version-26 cases and the real local and SSH host-update scenarios.
- `bun run build` in website, recorder `6de1fbd6-fdcf-429e-9b6b-6b489270f974`: passed, 33 pages, all internal links
  valid.
- `cargo build -p farhelm -p farhelm-fixtures`, recorder `8b2a65c8-149a-4ab5-977f-15ac825f9716`: passed.
- `dx build --package farhelm-ui --platform web --release`, recorder `6ee94900-d314-4427-88bc-4139aa445d25`: passed.
- `cargo clippy -p farhelm-helm -p farhelm-ui --all-targets -- -D warnings`, recorder
  `8ff964b7-031e-4bd8-a6ab-20e9a2fe6512`: passed.
- `npx playwright test 'settings\.spec\.ts|host-updates\.spec\.ts|tooltip-coverage\.spec\.ts'`, recorder
  `fab39a2b-307e-4cf2-8081-95fa1b40489a`: 94 passed, 47 per engine, none skipped or failed. This runs the full Settings,
  host-updates and tooltip-coverage specs, plus ntfy-settings because it shares the selected filename suffix.
- `cargo fmt --all -- --check`, scoped `dprint check` on both changed docs pages, `git diff --check`, and
  `python -B scripts/check-test-sleeps.py` with the isolated pinned parser environment: passed, 327 delays, none
  unexplained.

Historical evidence remains separate. UI tests at the original draft heads (#2807
`d0402cc2f8a5ef3137d7c5e22e5ed7ad4e58c6a1`, #2808 `7cdc86bd0629c99936fdc5329d52cb4a52ff1adb`) passed: 311
hosts/list/skew cases in `891f8f0e-a28b-44b9-936c-816e92ac3dc6`, and 91 preference/Settings cases in
`e17c2867-2593-4c80-8019-175eef3184f5`. These cover the unchanged classifier, dismissal and preference queue logic; the
rebased combined build and browser checks above cover integration with current main. The earlier changelog format check
remains applicable because the fragment is unchanged.

Earlier failed observations remain retained, not passes: the initial long-card browser attempt
`4b7052b0-e84e-46d6-9766-1630d0391161` (12 passes, two failures); the initial native compile
`144e31b9-541a-4704-b1a4-382f8b1ac406`; affected helm run `2beb4deb-b538-4db6-8fd2-64e26430942c` (434 passes, five
failures); and exact listener/fixture reproductions `8e271c2c-5533-4810-8120-de4b00202ced` and
`d3a336f3-8868-4cd8-9d77-b03fa5cf29a4`. Their corrected focused runs passed in the previous round. Interrupted
`daa2c59c-764d-4bae-877f-a59a72ffd3f2` and `d93a3dc6-0837-4f5b-ac13-61317236ba0d` supply no pass evidence. The later
landing failures, `43959a88-f565-461e-b3dd-760d8725cf4a` (two version-26 fixtures) and
`10af6125-cdc5-4afc-bc9a-369377c00195` (new switch hover text on both engines), prompted this round's corrections.

The current six recorder runs share source head `4936843aa2fbb2ad772b0f8c481d49b294886321` plus the four-file
correction, fingerprint `027b8c9940be69bacca3787b7016811ba95885074e3b0c35751ec7868b0338a8`. All have complete source
identity, complete reports where applicable, stream EOF, no output omissions and normal cleanup. Full retained-output
scans found no runtime skip marker. All three browser stacks cleaned up. Rust uses pinned nextest and tmux with four
global slots and zero retries; browser validation uses three workers, one owned stack each, both engines and zero
retries. No native desktop operation is claimed.

After those checks, the stack was rebased onto main `0f3d4bcdf236d897e34539c7c4c3dc030083fb7b`, preserving the terminal
zoom and new-terminal-button changes that landed meanwhile. The full upstream diff was inspected. The only conflict was
adjacent TODO removals; both sides' removals and main's new entries were kept. #2807's interdiff is empty; #2808 differs
only by that mechanical resolution plus the reviewed correction. Formatting and whitespace checks passed again after
resolution.

The helm tests and backend build remain applicable because no backend inputs changed. UI lint, web build and browser
coverage are reused for the unchanged host-update and Settings code: upstream changes only terminal toolbar visuals,
adds a display-only readout and keeps each control's tooltip attribute; none changes the selected browser specs or their
helpers. Those builds predate the toolbar changes, so they are not claimed as fresh tests of the new toolbar itself. The
website build remains applicable to the two corrected guides; upstream changed a separate page's prose without changing
links, headings or build configuration. The sleep check remains applicable because no delay or changed test premise was
added by the rebase. No additional runtime run was needed for TODO or commit bookkeeping.

Skipped: full workspace and browser batteries, unrelated JavaScript, desktop, installer, provisioning-distribution,
release and live-install checks. The requested helm crate and three browser specs cover this correction; the added docs
passages receive the website build. No preview server was started during this unattended run. No instruction or heading
rename requires the agent-instructions checker.

## Review gate outcome

The revised #2808 passed its required single fresh-context gpt-6.1-sol high review with no findings. The previous exact
Sol/high gate for #2807 and scope checkpoints remain applicable to their unchanged design. The current independent
resume assessment checked the complete upstream Podman and bookkeeping delta and the existing stack; a clean rebase
preserved both PR diffs. A new exact single fresh-context gpt-6.1-sol high review covers the revised #2808. Original
commit and PR wording is preserved from its accepted cold reads. A separate documentation pass covered all four
corrected files. Both PRs remain drafts for the landing monitor.

### Landing

Landed as redelivered, with one docs fix at landing (below). Host updates are easier to find: the Hosts button shows an
up-arrow when a host has an update, and the session list shows a card for optional updates or a banner when a host must
be updated before it can be used, each with a way to update or to open the host list. A new Settings switch under Hosts,
"Update remote hosts automatically", off by default, has the helm update remote hosts that run an older build or need an
update to connect, through the same path as the manual Update: when a host connects, when the setting is turned on, and
when a busy host finishes its work. It never updates the helm's own machine.

- #2807: the update prompts in the session list and on the Hosts button.
- #2808: the automatic update setting, where the helm stores it, and the docs.

Round 154 sent this plan back for two failing helm tests that check upgrades from an old settings database, a missing
hover text on the new switch and two docs pages. The redelivery fixes exactly those: the tests' rebuilt old-version
database no longer carries the new setting, the switch has hover text, and the Update guide and the Manage hosts page
describe the setting and the prompts.

#### What landed since the stack was based, and whether it interacted

Since round 154 checked this plan, terminal-zoom-icons and new-terminal-button landed (they were in that round's checked
stack too), plus documentation and TODO entries. Nothing interacted. menu-single-highlight landed right before this
plan; its menu spec drives the Update entry in a host's menu, whose state this plan now provides differently, and it
passed with both present (below).

#### Fixes made at landing

- The Manage hosts page said automatic updates work "even with no window open". On the Mac, closing the window quits the
  app and with it the helm that runs the updates, so the page now says the setting works whenever Farhelm is running,
  which on your Mac means while the app is open.

#### Things you should know

- The settings database moves to version 68 and cannot go back: a helm built before this plan, the v0.32.0 release
  candidates included, cannot open it afterwards. That matters for rolling back and for devbuilds.
- A remote host whose automatic update keeps failing is tried again on every reconnect.
- The same "even with no window open" wording, wrong on the Mac for the same reason, remains in the switch's own help
  and hover text in Settings (which the Mac app shows too), in SPEC.md, in the changelog fragment and in the executor's
  report. The landing left the first three alone because the plan and the specification chose them; the fragment will be
  reworded at release curation in any case.
- The helm reads the setting on every change to the host list, even when it is off; cheap, but new.

#### Checks

This round landed menu-single-highlight and host-update-discoverability, stacked in that order on main `ea62ecf3`. Round
154 ran the whole battery on both plans as first delivered, stacked with terminal-zoom-icons and new-terminal-button
(which have since landed). There the whole Rust workspace, run `43959a88-f565-461e-b3dd-760d8725cf4a`, failed only on
these plans' faults (fixed in the redelivery) and on known flakes. The whole browser suite on Chromium and WebKit, run
`10af6125-cdc5-4afc-bc9a-369377c00195`, failed on these plans' faults, on the known WebKit Restart with focus flake, and
on three cases that were neither: "plain Replace keeps a newer row's retry key after an older YOLO cancellation" and
"terminal link ownership opens once and preserves program clicks in all mounted terminals" on Chromium, and "launcher
gap keeps focus and container Enter explains an incomplete draft" on WebKit. Those three are rerun on this round's stack
below. On this round's stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint, `dprint check`,
  `python3 releasing/check-changelog.py format`, the docs website build (`bun run build` in `website/`), the workspace
  debug build and the UI's release web build: all passed.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `0d0ba293-25a8-40fb-884e-1c0997efbe07`: all 819 passed.
  The helm's tests, `cargo nextest run -p farhelm-helm`, run `671c9a89-dce7-4d07-8c6d-e67b38a4b91a`: all 1327 passed,
  including the two old-settings-database tests that failed in round 154. The UI's JavaScript tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `a508caa2-0dc3-45d3-bfda-02f1e52763b0`: all 372 passed, including the stylesheet
  test that failed in round 154.
- `npx playwright test` in `e2e/` with the hover-text sweep, settings, host updates, menu highlight and sidebar specs,
  on Chromium and WebKit, three workers, no retries, run `e0dec166-d73f-4d6f-a4cc-728ee3d9afab`: 343 passed, 1 failed:
  on WebKit, "the sidebar hides its scrollbar without giving up scrolling" timed out while deleting its 18 test sessions
  at the end. Repeated five times alone on WebKit, run `1f34fdef-94fa-48ee-a3ee-36b5fe3d5b6b`, it passed every time; one
  failure is not recorded as a flake.

- The three round-154 cases above, on this stack, Chromium and WebKit, five times each, no retries: the Replace case,
  run `a9df09fd-a447-495f-a4ae-ad2e845bd396`, 10 of 10 passed; the terminal link case, run
  `5c33ed32-2a83-4f09-a30c-491f80bab070`, 10 of 10 passed; the launcher case, run
  `723fd723-c996-48e2-bd61-cc119ddded35`, 10 of 10 passed. None is recorded as a flake on one failure.

- After the docs fix at landing (one paragraph on the Manage hosts page), `bun run build` in `website/` passed again.
- Reused from round 154 on these plans: the whole browser suite and the whole Rust workspace outside the reruns above,
  the desktop-build checks and tests, the binaries' lint, the desktop-asset check and the check that every test delay
  states its reason. The redeliveries change only a stylesheet test, a helm test's rebuilt old database, one hover text
  and two docs pages, none of them desktop code or test delays, and main has gained only the two plans round 154 checked
  these with, plus documentation and TODO entries. Skipped: the desktop smoke tests (no desktop-specific code changed)
  and the native Mac app (not runnable here).

The report's checks were the executor's selected runs; round 154 ran the whole browser suite and the whole Rust
workspace on this plan, and this landing reran the helm's and the UI's tests and the affected specs on the redelivery.
Apart from its "even with no window open" (above), nothing in the executor's report became untrue at landing.
