## What this was about

Hovering the version number at the top of the sidebar used to call it a client build and left a different helm version
unexplained. The hover now says plainly which Farhelm is running. Development windows identify themselves as development
builds; a mismatch names the helm and the window separately.

## Things you should know

The installed Mac app's idle hover explains that the readout turns red once a newer version is installed. Its
update-ready hover explicitly says that selecting it opens the menu for Restart to update or What's new. Checking,
installing, up-to-date, failure and reinstall messages use whole sentences; the reinstall command remains at the end
without trailing punctuation.

The visible version, colors, readiness markers, menu and click behavior are unchanged. Web windows and desktop builds
without an updater promise no update control. The existing shared development rule remains any SemVer 0.0.0 prerelease;
a version such as 1.2.3-dev.1 remains a release.

## Open questions and possible follow-ups

No decision or follow-up is required. The agreed idle wording is retained verbatim; selecting a red readout opens its
menu, and the ready hover spells that out.

## The PRs

[#1733](https://github.com/scode/farhelm/pull/1733/changes) — plain version hover text for every window and updater
state. Draft, pushed at `877130dec60b9f52fdf98ea8c87649effe00db6d`; not marked ready or merged.

## Checks run, reused and skipped

Recorded shared Rust selection `09f1e1b8-8ded-419d-8d1e-a7520b8eefa7` passed six tests for the shared hover selector,
development classification and helm release comparison. Recorded desktop selection
`309bf0b2-457a-47ea-89f0-2dad851a2955` passed forty tests for updater state and hover behavior. Four nextest slots, zero
retries. The reports contain 1577 and 507 selected-out JUnit skips respectively; no runtime SKIPPED messages. Both runs
have complete source identity, output and JUnit evidence, with no failures, flaky results, output truncation or forced
cleanup.

The exact changed sidebar readout case passed on Chromium and WebKit in recorded run
`c7852c83-aefe-4964-9765-10af59ef9c0f`: two passes, one worker, zero retries or skips, complete source/report/output and
cleanup. It covers matching development builds and a different helm stamp reaching the visible readout and hover.
Release-only desktop texts are covered by the Rust selections rather than the development browser bundle.

Earlier browser run `be42dc00-8cc4-4978-bcdc-8be93f9006d0` passed Chromium and failed WebKit before the new hover
assertions: the unchanged initial readout assertion allowed only five seconds after navigation. An unchanged exact
WebKit reproduction, run `042d9e34-5ff8-4b24-ab7d-b3bda69c8c4a`, failed two of three attempts. The trace shows first API
replies completing after that readiness deadline and the expected readout appearing afterward. Both failures and
attachments remain retained. The test now waits for the exact expected stamp using the established 20-second sidebar
startup budget at both navigations, with a live helm probe on failure; hover assertions retain five seconds. FLAKES.md
records this inherited readiness issue and the limits of the evidence. No original-product baseline was rebuilt, and CPU
causation is not established.

Workspace all-target Clippy, shipped-binary Clippy, desktop-feature all-target Clippy, desktop UI compilation, native
build and release web build passed. Rust formatting, relevant Markdown/TOML dprint and changelog-format checks passed.
The isolated source-delay checker inspected 269 delays with zero missing reasons. Dprint does not select TypeScript
here; no TypeScript formatting verdict is claimed. A separate documentation pass covered every touched file.

Reused each successful command after the careful rebase from base `46f78dcb` to main `466792ab`. Every intervening diff
was inspected: repository caching, desktop UTF-8 declarations, conversation-warning behavior and persistence, and
plan/TODO bookkeeping. They do not change version classification, the hover selector or updater contracts. Shared Rust
evidence records the original implementation diff; desktop Rust and builds record `3598ae3c`; corrected browser evidence
records that commit plus the exact folded readiness correction. Those feature files remain byte-identical after rebase,
and the final addition is only the flake documentation. Rust formatting, relevant dprint and changelog lint passed again
after rebase; the appended flake entry also passed dprint. All upstream TODO entries and links were preserved.

Skipped full workspace/desktop runtime and full browser batteries because the targeted selections cover the affected
text selection and updater states. Installer, provisioning, desktop smoke, asset-set parity, website and hosted CI
checks add no evidence for this change: installation, assets, update actions and the website are unchanged. No
deployment or live-install mutation was requested.

## Review gate's outcome

The required sole gpt-6.1-sol high source review found no defects against the agreed texts, existing update behavior and
full test-authoring checklist. The same required model and effort were requested for a fresh supplemental review of the
browser readiness correction; it passed with no findings after an initial task-transport interruption was resolved. The
executor inspected both complete artifacts and the diff. Commit and PR wording passed a separate gpt-6.1-sol medium cold
read; the PR body is empty. Implementation and investigation stayed with the executor. Native model-reporting and usage
counters were unavailable, so exact reviewer attribution cannot be independently verified; private galaxy session
evidence records that gap.
