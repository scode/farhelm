## What this was about

Agents dispatching work on their own host previously needed approval for each session change or the broader permission
to act across the fleet and change launch templates. This plan adds **Manage sessions on this host without asking**, off
by default on existing and new hosts, including Tensorlake sandboxes.

You can turn it on in host settings. Cards for the covered same-host session actions also offer **Always allow on
&lt;host&gt; only**, except the command-prefix cases below. It covers creating, spawning, copying, renaming, stopping,
restarting and deleting sessions on the requesting host. A copy's source must be on that host too. The broader **Run
farhelm commands without asking** remains independent; either switch keeps its own saved value while the other is on.

## Things you should know

The helm still decides and must be reachable. The narrower grant supplies no offline authority and covers no requests
for other hosts, template changes, host changes, links or messages to existing agents. Adding, editing or removing a
host always needs approval. A command prefix supplied by an agent still needs its own Allow on a host that asks before
YOLO launches; writing a template containing such a prefix always asks. YOLO and custom-command create, copy and spawn
still need the separate YOLO permission; a plain restart, including of a YOLO session, keeps its existing exemption.

The card grants permission only to the installation that asked. Adopting a replacement host resets it. Other cards
already waiting still need their own answers. Older clients retain their existing buttons, and an older helm rejects the
new answer rather than interpreting it as fleet-wide permission.

The printed agent instructions and the **Approve what agents do**, **Manage hosts** and **Security model** pages explain
the permission. The security-model page remains a stub. The completed TODO entry is removed.

## Open questions and possible follow-ups

No product decision remains. The existing host-settings screenshot can be recaptured separately to show the additional
switch; its image was outside this change.

## The PRs

- [#3205: backend permission](https://github.com/scode/farhelm/pull/3205/changes).
- [#3206: host settings switch](https://github.com/scode/farhelm/pull/3206/changes).
- [#3208: approval-card choice](https://github.com/scode/farhelm/pull/3208/changes).
- [#3211: agent instructions, documentation and TODO completion](https://github.com/scode/farhelm/pull/3211/changes).

All four are open drafts in that order, one commit each. Their heads are `50b125e9`, `7bb6f0e3`, `e123a9b9` and
`1db7a56b`.

## Checks run, reused and skipped

- Passed the initial focused helm selection, run `05081c94-d202-4217-9714-3b09228c2443`: 40 tests, including the 48-case
  checks of every covered verb with same-host and other-host targets, exclusions, defaults/upgrades/resets, independent
  settings writes and the host-listing contract.
- Passed host-settings UI and host-field decoding, run `cf4d3c65-0419-40ad-8e49-e516e466a47d`: four tests.
- Passed combined protocol, helm approval lifecycle, grants tied to the asking installation and UI approval/settings
  coverage, run `3decca51-d54b-40f0-9c9d-c2a9f048c9b9`: 75 tests. All three runs used pinned nextest, four slots and
  zero retries; source and reports are complete, retained output is untruncated, and no runtime substrate skips were
  reported.
- Passed targeted proto, helm and UI all-targets clippy with warnings denied, Rust formatting, selected Markdown/fixture
  formatting, changelog format, and the source-only test-delay check (423 delays, none unexplained). The instruction
  reference checker passed with one pre-existing triage-heading warning.
- Passed the website frozen install and build: 36 pages built and all internal links valid. The existing empty-i18n and
  missing-404-content warnings did not fail the build. No preview server was started for this unattended plan.
- Passed `cargo nextest run -p farhelm --bin farhelm --test e2e` with a filter for the printed-instructions module and
  the real-helm lifecycle, creating and keyed-create/clone-after-delete cases, run
  `1ea78a4c-2c7e-4e66-94dd-ce33022c4db2`: 13 tests plus successful fixture setup. Pinned nextest and verified pinned
  tmux, four slots, zero retries; complete source/report/output and cleanup, no runtime substrate skips.
- Passed matching `cargo build` and pinned DX web release builds, then
  `npx playwright test 'spawn\.spec\.ts' 'approval-layout\.spec\.ts' 'terminal-multihost\.spec\.ts' 'keyboard-scroll\.spec\.ts'`
  with the focused grep selection, run `d3628881-2006-49ea-88d0-65563dc43869`: 16 passed, eight per engine, no skips or
  failures. This covers granting same-host authority then spawning again without a card, existing Allow/fleet-wide
  Always allow/Deny, four-button long-host layout, stored switch independence and keyboard focus. Three workers with one
  stack each, zero retries, verified pinned tmux; complete source/report/output and cleanup. Provisioning and Tensorlake
  are simulated by the browser harness; the helm, supervisors, CLI and fake agent are real.
- Reused these checks after the final rebase onto `295b29fa`. All incoming changes were TODO and plan records, with no
  product-code or contract interaction. Each of the four review units has an empty interdiff against its reviewed
  version. The 75-test run covered PR3's working tree `90276539`, committed as `3a1c057c`; the later CLI, browser,
  website and source checks covered PR4's working tree `cc52078d`. Their patches are unchanged in the final heads above.
  Initial backend and settings results cover the earlier review units; the later card changes received their own
  combined coverage. The one earlier PR1 amendment corrected a specification permission count only. A targeted TODO
  formatting check also passed after rebase. No repeated runtime run was needed for bookkeeping changes.
- The portable retained-run summary confirms five completed successful commands with complete discovery. There were no
  failed runtime attempts in this plan. No CI was dispatched; it is manual-only.
- Skipped the full workspace/browser batteries, native desktop, installer, release and real vendor-agent checks: the
  change concerns helm permission decisions and shared UI, with focused coverage selected for those boundaries.

## Review gate's outcome

Each of the four PRs passed the required single fresh-context gpt-6.1-sol reviewer at high effort with no actionable
findings. Commit wording passed independent cold reads. No scope expansion or substitute reviewer was needed.
