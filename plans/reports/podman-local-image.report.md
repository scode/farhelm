# Local Podman images

## What this was about

Starting every containerized session from an archive makes Podman unpack the image even when its layers are already
stored locally. The maintainer chose an image already loaded or built on the session's Linux host, with no image
management or launch-time benchmark added to Farhelm.

The launcher now asks for **Image**, a local name or ID. The host checks local storage before admitting the launch, and
creation disables pulling. A missing image refuses with its name and an explanation that it is not in Podman's local
storage on this host. Each new container resolves the name again; rebuilding the same tag affects the next New, Replace
or Replace with, while existing containers retain their original image.

## Things you should know

Archive settings saved by earlier release candidates remain containerized but read with an empty image. Existing
sessions and templates still load, and old templates still apply and can be edited or saved. A new launch needs an image
filled in; use Replace with for an old session. No archive is converted or imported. Protocol 67 requires the normal
coordinated helm/supervisor update; no database migration was added.

Stop, Delete, terminal tabs, the managed-checkout mount, proxy port, token delivery and creation fence retain their
behavior. Delete leaves images alone. The incidental docs update is the Containerized sessions page; no preview server
was started during this unattended run.

Release-note curation consumed the original container fragment during execution. The retained fragment now covers
local-image selection and its saved-settings caveat; the upstream draft release notes are preserved.

## Open questions and possible follow-ups

No decision is needed. Automatically preparing an image on a remote host remains separate planned work.

## PRs

- [#2825 — launch containers from local Podman images](https://github.com/scode/farhelm/pull/2825/changes), one draft
  PR.

## Checks run, reused and skipped

- Recorder run `24eaf74b-5f66-4fc1-8fcd-650ea68d007a`:
  `cargo nextest run -p farhelm-proto -p farhelm-supervisor -p farhelm-ui --lib -E 'test(container)'` passed 27 tests.
  Its supervisor coverage is reused from base `b317c4c9` plus the recorded working-tree diff: later corrections changed
  UI/proto validation and equivalent token-check syntax, not the exercised supervisor behavior.
- Run `bababcb7-d6b6-4b6a-98f4-bd937832cd35`:
  `cargo nextest run -p farhelm-proto -p farhelm-ui -p farhelm-helm --lib -E 'test(container) | test(protocol_version_is_pinned_at_67) | test(flags_edit_the_templates_result_and_win_over_it)'`
  passed 16 tests. This covered the corrected tag handling and launch-only required-image gate. Both unit runs used four
  nextest slots and zero retries.
- Run `f624dbb2-7f17-4467-9e34-b3ebda29d42c`:
  `cargo nextest run -p farhelm --test e2e -E 'test(containerized_sessions::)'` passed both real rootless Podman tests
  and fixture setup, with the offline Ubuntu image and no runtime opt-out. Isolation, missing-image refusal,
  Stop/Delete, tabs/downloads and failed retry ran in scoped and unscoped modes.
- Run `56113165-b963-4417-81e9-9a0257069f60`: from `e2e/`,
  `npx playwright test tests/templates.spec.ts tests/github-checkout-composer.spec.ts -g 'container choices require a managed checkout and reach the create intact|container templates edit, apply, save and explicitly clear the complete choice'`
  passed all four executions, two per engine, on Chromium and WebKit, with three workers and zero retries. Existing
  mobile launcher/template screenshots were inspected. No runtime failures or skips occurred; nextest's other cases were
  selected out.
- `cargo fmt --all -- --check`, clippy for the five affected crates with `--all-targets -- -D warnings`, ordinary CLI
  and web builds, changed-Markdown dprint, changelog format, the website build and internal links, and the isolated
  sleep checker all passed. The final sleep scan inspected 327 delays with none unexplained.
- The initial lint attempt found a nested `if` that clippy required collapsing; its equivalent let-chain correction
  passed the rerun. One intermediate source-only sleep scan exceeded its 60-second budget under shared load; the final
  scan completed successfully. These were not runtime test failures.
- Runtime results and the web/website builds were reused after the final mechanical lint correction and
  release-note-only rebase because the tested behavior and edited docs page were unchanged. No additional runtime run
  was justified. Full workspace, full browser, desktop, installer, provisioning and CI runs were skipped because this
  change is confined to container-image selection and the targeted checks cover its integration risks. No launch
  benchmark was run, as directed.

## Review gate outcome

The required single fresh-context gpt-6.1-sol/high review found one issue: an ordinary local tag such as `docker:latest`
was being mistaken for a transport prefix. That was corrected and covered by a unit case. The same reviewer checked the
correction and the launch-only required-image gate, then reported no blocking findings. The commit and PR wording passed
its separate cold read.

### Landing

Landed with fixes at landing that mark it as a breaking change (below). Containerized sessions now start from an image
already in the host's local Podman storage, named in the launcher's **Image** field, instead of from an image archive
file, so Podman no longer unpacks an archive on every launch. Farhelm never pulls or manages images, and it refuses an
image that is missing from that host's local storage. Settings, templates and sessions saved with an archive path show
an empty **Image** and must get one before they launch again (edit the template, or use **Replace with** for an existing
session); no archive is converted or imported.

- #2825: the image field across the launcher, templates and container settings, the supervisor's container start and its
  stored settings, their tests, the specification text, the Containerized sessions docs page and the changelog entry.

#### What landed since the stack was based, and whether it interacted

Since the plan was based, the keyboard shortcuts overview (#2827) and the Settings gear shape fix (#2826) landed;
neither touches containerized sessions, and nothing interacted. typed-quick-launch (launching templates by typing their
names), stacked on top of this plan in the same round, was sent back to its executor; a review found the two plans'
template changes independent.

#### Fixes made at landing

- The plan assumed containerized sessions had shipped only in release candidates, but v0.32.0 was published as a stable
  release on 2026-10-30 with the archive-path **Image file** field. Replacing it is therefore a breaking change for
  stable users. The changelog entry is now `kind: breaking` and says what users must do; the docs page now says
  "settings saved by earlier versions" instead of "by release candidates"; the PR and its commit are titled `feat!:`,
  and their description, which said container settings had shipped only in release candidates, now says the change
  breaks settings saved by v0.32.0.

#### Things you should know

- The change is one-way: v0.32.0 requires the archive path and refuses fields it does not know, so it cannot read any
  containerized session, template, recent setup or container setting that this version creates or rewrites; going back
  to v0.32.0 would leave those unreadable. The protocol moves from 66 to 67, so hosts need the usual update together
  with the helm.
- Save as template shows an old archive-era setting's summary with an empty image (`Podman ·  · port` and the session's
  port); cosmetic.
- The executor's report calls the old format "earlier release candidates"; v0.32.0 stable has it too.

#### Checks

This round first checked this plan with typed-quick-launch stacked on top, on main `ce756c29`. On that stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint, the binaries' lint,
  the desktop-build checks, the check that every test delay states its reason
  (`python -B scripts/check-test-sleeps.py`), `dprint check`, `python3 releasing/check-changelog.py format`, the docs
  website build, the workspace debug build, the UI's release web build and `scripts/check-desktop-assets.sh`: all
  passed.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `fa7dd3b8-99bf-43ef-9d7b-e4dbca885186`: all 832 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `d12ee8fc-fcf3-4724-b807-362a166bfabd`: all 1015 passed. The UI's JavaScript tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `8c0811c6-00f9-4db5-ac73-f524165f8984`: 373 passed, 1 failed on a stylesheet
  literal typed-quick-launch added.
- `npx playwright test` in `e2e/`, the whole browser suite on Chromium and WebKit, three workers, no retries, run
  `f69ef2f8-60e8-4dd5-9413-5380dc2deb9b`: 2835 passed, 14 skipped by design, 1 failed: on WebKit, "expanded setup
  failure closes the menu below it", in host setup code neither plan changes. Rerun five times alone with both plans
  present it failed once (run `045bd6bb-48df-4561-b9a4-b5ca5daa702e`), and with this plan alone it passed five of five
  (below); it is not recorded as a flake yet, and typed-quick-launch's executor is to run it on main if it fails again
  with that plan.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, on a machine with rootless Podman, run `c801e422-448d-45a7-8bfe-cda03b3e8874`: 4573 passed, 41 skipped by
  design, 1 failed: the known flake "shallow pause resumes without reset or replay". The containerized-session tests ran
  against real Podman rather than returning early.

With this plan alone on main `ce756c29`, after the landing fixes:

- `dprint check`, `python3 releasing/check-changelog.py format`, the docs website build, the workspace debug build and
  the UI's release web build passed. The UI's JavaScript tests, run `454db66e-59a5-4606-a1dd-ca4d8d87158e`: all 372
  passed.
- `npx playwright test` in `e2e/` with the templates and GitHub checkout composer specs the plan edits, on Chromium and
  WebKit, run `1056a7a1-98f4-4e68-a728-36946d3f6c2f`: all 172 passed. "expanded setup failure closes the menu below it",
  five times alone on WebKit, run `3f758104-5642-4651-b4f8-2bada4250513`: all passed.
- Reused from the stacked run for everything else: removing typed-quick-launch took code away and added none, and the
  landing fixes change only the changelog entry and a docs sentence. Not run: the desktop smoke tests (no
  desktop-specific code changed), the native Mac app (not runnable on Linux; containers are Linux only), doctests, the
  supervisor-asset and website JavaScript harnesses, installer, uninstall and provisioning-script tests, the release
  configuration check and the scripts' self-tests, none of which covers what this plan changes.

The report's checks were the executor's selected runs; this landing ran the checks below with this plan stacked under
typed-quick-launch and reran the plan's own specs on it alone. Apart from the release-candidate wording above, nothing
in the executor's report became untrue at landing.
