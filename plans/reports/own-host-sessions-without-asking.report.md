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

### Landing

Landed with fixes made at landing. A host can now let agents in its own sessions create, spawn, clone, rename, stop,
restart and delete sessions on that same host without an approval card, through **Manage sessions on this host without
asking** in the host's settings or **Always allow on &lt;host&gt; only** on such a card. It starts off on every host,
and requests aimed at other hosts still ask.

- #3205: the permission itself, its storage and the helm's checks.
- #3206: the switch in the host settings dialog.
- #3208: the approval card's **Always allow on &lt;host&gt; only**.
- #3211: the docs pages.

#### What landed since the stack was based, and whether it interacted

prepared-snapshots (#3203) landed just before this round and took the helm database's next version for its own addition,
which this plan had also claimed. The landing gave this plan's change the version after it, so the two upgrade steps run
one after the other; the review checked every place that depends on the order. The round also landed omp-npx-docs and
snapshot-in-place-paint, which share nothing with this plan. stop-start-message, which also touches how sessions stop,
did not land this round; the review found no shared code with it. Main's six test-only PRs (#3215 to #3220) scope test
locators and do not touch this plan's paths.

The review also checked that the same-host test cannot be fooled. It compares the host the helm itself resolves for the
target, never a name, alias or anything the requesting host claims. Changing hosts, templates and links never counts as
same-host. The card's new answer works only on a card that qualifies, and only while the host still has the identity
that asked. Adopting a new identity clears the grant.

#### Fixes made at landing

- **Upgrade order.** As above, this plan's upgrade step now runs after prepared-snapshots' instead of colliding with it.
  Both steps only add a column (this one defaulting to off), so no existing data is at risk.
- **Specification.** One SPEC.md sentence still said adopting a new identity resets four permissions; it resets all five
  now, as the plan's own edit elsewhere in SPEC.md already said.
- **Changelog entry.** It opened as if agents now act without asking by default; it now says the permission is off until
  you turn it on, and names both ways to turn it on.

#### Things you should know

- A Stop an agent issues on its own host is still recorded as "stopped by user". That predates this plan, but this
  permission will make such stops more common; recording who stopped a session is a possible follow-up.

#### Checks

At the maintainer's request the browser specs and the Rust tests ran on Tensorlake sandboxes; the builds, lints and the
UI's tests ran on the development machine. They ran on the round's four plans stacked in queue order on main `ec2e4984`
(own-host-sessions-without-asking, stop-start-message, omp-npx-docs, snapshot-in-place-paint):

- Locally: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`, the test delay check (423 delays, none
  without a reason), `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`
  (no errors), `cargo build`, the release web build and the docs website build: all passed. The UI crate's tests in its
  desktop build (`cargo nextest run -p farhelm-ui --features desktop`), run `bab8b05a-205f-451a-b862-62da7102602c`: all
  1109 passed.
- Browser specs on eight sandboxes, both engines (`approval-layout`, `remembered-permissions`, `agent-relay`,
  `yolo-guard`, `spawn`, `terminal-multihost`, `keyboard-scroll`, `settings`, `modal-layout`, `terminal`, `stop-tabs`,
  `terminal-restart`; `scripts/tensorlake/sandbox-tests.ts`, each shard through the recorder's generic mode with
  `npx playwright test --project=...`, three workers, no retries; the sandboxes run Node 24.16 against 24.20 locally).
  Shard runs `60e17368-0041-4586-87f1-20a5ecbf5e27`, `4b2c0b69-c923-47db-8551-624e3c172738` and
  `61086004-e0e6-4413-800f-9527624fd7b8` each had one failure; the other five shards passed. Two failures were the
  terminal spec's "stop and delete failures surface in the row's own error line" on both engines: main's #3194 added a
  hidden copy of a row's error line, and #3220, which landed during the round, fixes that test; no plan here touches it.
  The third was WebKit's "clones onto the other host with the source's own command" (agent relay), a 30-second wait for
  the clone; a rerun of that group, three times on both engines, run `8dc2d88b-61b1-413d-a4dc-8e74000ead8f`, passed all
  24. That one failure is kept as evidence, not recorded as a flake from one observation.
- Rust tests on ten sandboxes: a `cargo nextest archive` of the helm, protocol, supervisor, terminal emulator and
  farhelm crates (every unit and integration target, the Podman container tests included), split into ten hash
  partitions, each run through the recorder's generic mode with pinned nextest 0.9.148, four slots, no retries. About
  4,660 tests ran; four failed. Three were stop-start-message's: on these faster machines a test's Stop now often lands
  before the agent has started, which that plan correctly records as "stopped before the agent finished starting" while
  the tests still expect "stopped by user"; it went back to its executor to fix the tests and did not land this round.
  The fourth, `host_creates::managed_github_token_is_fresh_for_agents_tabs_and_restart` (shard run
  `06dce18d-2dc3-4ae3-bc7c-fcf350a9ecc4`), passed in a narrow local rerun, run `0574a2a8-1b27-4ba8-a72d-ffd21c8cc24c`;
  it is the first run of these tests on sandboxes, and one sandbox-only observation is not yet a flake.
- After the checks: stop-start-message was taken out of the stack, the review's wording fixes went in (documentation and
  a changelog entry), and the merges rebased the PRs onto main's six test-only PRs (#3215 to #3220). Nothing was rerun
  without stop-start-message: the tested tree included it, and the review found no code it shares with these plans, so
  the runs above still cover them. #3220's fix of the terminal spec was not run together with these plans. Main outside
  `plans/` matches the tested tree minus stop-start-message, plus those six PRs' spec files. `dprint check` and the
  changelog check passed again after the fixes; the docs website build ran before the documentation fixes and was not
  rerun.
- Not run: the rest of the browser suite, the other Rust crates, doctests and the native Mac app.

The executor's report is otherwise still accurate, apart from the PRs' draft state; they are merged.
