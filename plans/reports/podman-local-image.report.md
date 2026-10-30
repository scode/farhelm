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
