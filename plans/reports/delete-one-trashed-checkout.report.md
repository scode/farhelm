## What this was about

The trash dialog could permanently remove every archived checkout on one host or across all hosts, but could not remove
just one. Each checkout now has its own delete button so you can clear one archive while keeping the rest.

## Things you should know

The new button deletes immediately, with no confirmation or warning about uncommitted or unpushed work, as requested.
Per-host and global deletion still ask for confirmation. Issue notices without a checkout row have no individual delete
button and remain part of bulk cleanup.

An individual delete needs the same settled, current host listing as bulk cleanup. It stays unavailable during another
deletion, another operation holding the shared guard, a failed host-list refresh, or a host listing above the
10,000-entry cleanup limit. An unreachable host has no checkout entries to delete. The request contains only the clicked
checkout's record ID and the host connection observed at that click; the existing supervisor ownership checks still
apply.

The header and affected host announce deletion while it runs, and Close and Escape remain unavailable. The reply removes
confirmed records or shows the host's failure, then refreshes the listing and focuses Close. A lost reply still warns
that some contents may already have been deleted. The single action shares this completion path with bulk deletion.

The specifications now include immediate single deletion in the permanent-cleanup exception. No helm, supervisor or
protocol changes were needed. The covered TODO entry is removed, and a release-note fragment records the new action.

## Open questions and possible follow-ups

None required for this plan.

## The PRs

- [#1888](https://github.com/scode/farhelm/pull/1888/changes): delete one checkout from the trash; draft, based on main.

## Checks run, reused and skipped

- Passed `cargo check -p farhelm-ui --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo build`, and the
  release web build. These compile and lint the shared dispatch and prepare the real browser stack.
- Passed `cargo fmt --all -- --check`, changed-Markdown `dprint check`, `python -B scripts/check-test-sleeps.py` with
  the isolated interpreter, and `python3 releasing/check-changelog.py format`. The delay checker found 287 delays and
  zero missing reasons.
- Passed the focused UI trash selection tests through the recorder:
  `cargo nextest run -p farhelm-ui --lib -E 'test(list::trash::tests::)'`, run `a8ad6cf3-660a-4d2a-b61f-700bd8e3d0d9`: 2
  passed; 447 unrelated tests excluded by selection. This checks exactly one record ID, current host authority,
  neighboring and issue-only exclusions, and whole-host eligibility.
- Passed the complete `trash-dialog.spec.ts` on Chromium and WebKit, run `f657dee7-091b-4cb6-9816-5d5cf99f4007`: 18
  passed, no skips. Controlled replies exercise immediate deletion, failure recovery, progress, admission, focus and
  Escape protection, plus existing bulk confirmations at ordinary and phone widths.
- Passed `github-checkouts.spec.ts -g trash` on Chromium and WebKit, run `86f54fd5-96f9-45e5-847b-e139e8aa6d68`: 6
  passed, no skips. Real local-supervisor cases cover bulk cleanup and single deletion that preserves its neighbor's
  complete contents, including uncommitted work.
- Browser runs used the recorder, pinned tmux, one worker and zero retries. The initial run
  `3a6de622-1cec-4c9b-bd2d-8476db44a07f` failed all 18 browser launches because the recorder cleared the container's
  browser-path override; no product assertion ran. The installed browsers were made discoverable at their default
  location. A narrow recorded success-case retry, `eb698c41-6330-4c68-b468-47998e71e836`, passed both engines before the
  complete selected runs above. The failed evidence remains private; no latent flake was established.
- The builds and successful checks cover source snapshot `9b288b61`; pushed head
  `3d7626b7107c4105f11aadb4dc023be6adb1ad97` changes only its commit description. A fresh fetch at main `5ca8596d` found
  only another plan's claim and six report approvals since the base, with no product or specification changes. No rebase
  or repeated runtime checks were needed.
- Skipped full Rust/browser batteries, desktop-shell runtime, installer, provisioning, website and release checks: this
  changes the shared trash UI and its tests, with no backend, protocol, desktop-only, installation or website behavior.
  The selected checks cover the concrete selection and dialog risks. The stack's SSH-to-localhost fixture was
  unavailable; these cases exercise the real local supervisor and controlled remote listings, and do not claim real
  SSH-host coverage. No hosted CI or deployment was requested.

## Review gate's outcome

The required fresh source reviewer found no product defect. It identified a test that needed explicit dialog focus
before pressing Escape during a held deletion; that fixture premise was corrected and accepted. Runtime evidence is
reported separately above. The commit and PR wording passed its separate fresh reader.

The executing agent did all implementation work. Separate agents checked the source and wording; the final report
receives its prescribed native cold read before delivery. Written findings are retained and read by the executing agent.
The source reviewer was requested on GPT-6.1 Sol at high effort and the wording reader at medium effort. The harness did
not expose independent confirmation of the actual models or their usage, which limits model attribution and cost
accounting; the written findings and recorded runtime results remain separate evidence. Private galaxy-brain session
`5c9810ab-dcb4-4a62-9a79-bfb239f2d810` retains the routing and review records. The working log beside the checkouts
records its exact location and the retained run directories.
