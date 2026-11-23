## What this was about

In Settings → Tensorlake → **Sandboxes not on this helm**, a failed refresh could hide sandboxes that still needed
deletion. If some cleanups were already running, the list could show only those rows and put the refresh error on each
of them.

Settings now keeps the last successful list and shows the refresh error above it, as requested. Cleanup status continues
to update during the outage. When cleanup takes over a sandbox's row, its older discovery row is forgotten, so a
completed cleanup disappears without bringing that older row back. A successful refresh replaces the list and clears the
error.

## Things you should know

The 20-second discovery deadline covers waiting for the saved key and checking which sandboxes the helm owns, as well as
waiting for Tensorlake. A stalled local read can therefore report its own error and still return the cleanup snapshot.
If Settings has yet to discover any sandboxes, failure shows the error without claiming the account is empty.

## Open questions and possible follow-ups

None.

## PRs

- [#4675](https://github.com/scode/farhelm/pull/4675/changes) keeps known Tensorlake sandboxes visible while discovery
  is unavailable.

## Checks

- Ran `dprint check SPEC.md SPEC_impl.md TODO.md releasing/changelog.d/tensorlake-leftovers-discovery-failure.md` and
  `git diff --check` on the rebased change. Both passed.
- Reused the pinned `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-gui --lib` with the Tensorlake
  discovery, provisioning and Settings filters at `1a13c5e70fbd`, run `caa9e360-c9ab-4dd7-bcc4-1b1d73bfdc98`. All 73
  selected tests passed. The helm cases used an owned SQLite store and fake Tensorlake, and Settings used a fake helm.
  The cases cover failure, cleanup starting and finishing during an outage, overlapping refreshes, deadline, key clear,
  Delete and recovery.
- Reused `python3 scripts/check-gui-goldens.py --scene 'settings-tensorlake*' -j 4`, run
  `a61bc2a1-d0f8-4eae-adab-012c4ef89bf0`, and `cargo clippy -j 4 --all-targets -- -D warnings`, run
  `c6ad0c9d-84b0-48ad-a51c-1ec3a30fc05e`, at `1a13c5e70fbd`. The selected reference renders matched and Clippy passed.
  `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, and
  `python3 releasing/check-changelog.py format` passed at the same revision.
- Main gained the independent Suspend and Resume routes change and bookkeeping updates. The rebase leaves the discovery
  and Settings patch intact. Resolving the adjacent TODO removal preserves main's changes and removes only this plan's
  entry, so the earlier checks still cover the rebased change.
- Skipped the full test and reference render suites because the targeted checks cover the affected workflow and its
  reference renders matched. Skipped live Tensorlake because the local fixture can reliably trigger failures and
  timeouts while cleanup runs.

## Review gate

An independent source review found that overlapping refreshes could leave a deleted sandbox listed during an outage.
Settings now remembers that cleanup began even if it has already requested another refresh with the same saved
Tensorlake API key. The review also led to coverage for cleanup starting during an outage, a direct check of the alert's
position above the rows, and the deadline for local discovery reads. All four findings were addressed and the revised
change passed the checks above.

### Landing

#### What landed with it

The plan's one PR (#4675) landed in this round right after remove-own-setting-permission (#4669, #4672, #4673 and the
landing fix #4685), which changes how saved agent launches are classified and touches nothing in the Tensorlake leftover
list. Since the plan's base, main had also gained the session list's notice for a sandbox's Stop and suspend action that
runs past 20 seconds, and two TODO.md additions. The rebase met TODO.md, where this plan's removal of its own entry sat
next to the Stop and suspend entry that plan had just removed (both stay removed), and one paragraph of SPEC_impl.md
about replies the helm limits to 20 seconds, where the Stop and suspend plan had reworded the text around this plan's
sentence about the leftover list. The resolution keeps that rewording and this plan's sentence; the independent reviewer
confirmed both.

The landing's independent reviewer found the plan's new reply from the helm to Settings (the discovery error beside the
current cleanup snapshot), the 20-second deadline covering the local key read, Tensorlake's answer and the ownership
check, and Settings' handling of the list across refreshes, cleanup and Delete correct. The desktop app only ever talks
to the helm built with it, so the new reply shape needs no version check.

#### Fixes made while landing

None. The landing fix the checks below mention (#4685) belongs to remove-own-setting-permission.

#### Checks

This round landed remove-own-setting-permission (how saved agent launches are classified, a supervisor database upgrade,
the helm's launch history and YOLO question, the desktop launcher, Replace and permission marks, reference pictures and
docs) and tensorlake-leftovers-discovery-failure (the helm's Tensorlake leftover list, a new reply type and Settings)
together. Between them they reach every crate except the desktop app's outer program, so the landing ran the whole
workspace suite, the lints (the desktop app's too), the end-to-end smoke test, every reference picture and the docs site
build on both plans as delivered, stacked on main, then narrower runs after the landing fix and after a mid-round change
to main. The worker runs were on a separate arm64 Linux test worker with a systemd user manager, rootless Podman and SSH
to itself; the rest ran locally on x86_64 Linux. None of them used a real agent program or a real Tensorlake account:
the tests use stand-ins, as the reports say.

On both plans as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `13298a6c-7f5f-46ce-a761-2caa08b85278`): 8413 of 8414 passed and none printed a runtime `SKIPPED`. The failure is a
  test of the desktop launcher's recent setups that remove-own-setting-permission broke, which that plan's landing fix
  (#4685) corrects; the failed run is kept.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's two
  `cargo clippy -p farhelm-desktop --all-targets` runs (with and without `--features agent`), on the test worker:
  passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `4e8cb1fd-10e7-4211-8f0a-d30516dcbb4d`): of
  its 50 steps, 49 passed and one was skipped as always on Linux (paste, which headless Mutter cannot give the app).
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `768fb235-2adc-486f-a06c-ed76fcf489bf`): every picture
  matched its reference, including the pictures remove-own-setting-permission added or replaced.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format` and the docs site build (`bun run build` in `website/`), locally:
  passed.

After remove-own-setting-permission's landing fix (#4685):

- `cargo nextest run -p farhelm-gui -p farhelm` with the desktop launcher's and the agent instructions' tests selected,
  on the test worker (recorder run `85f1444a-54cb-4501-9740-6b6cafaf5ea6`): all 548 passed, the corrected test among
  them.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `python3 -B scripts/test-gui-screenshot-batch.py` (the screenshot scene list's own checks, locally and not through the
  recorder, so it has no run id): all 47 cases passed. `cargo fmt --all -- --check`,
  `python -B scripts/check-test-sleeps.py`, `dprint check` and `python3 releasing/check-changelog.py format`, locally:
  passed.

After remove-own-setting-permission merged, main also held a change to the opt-in real-agent tests that landed mid-round
(#4674), which none of the runs above had seen. On main plus tensorlake-leftovers-discovery-failure:

- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed, so every test target still compiles.
- `cargo nextest run -p farhelm -p farhelm-supervisor` with the real-agent test module's guard tests and the
  supervisor's agent and persistent hook tests selected, on the test worker (recorder run
  `eb6f15f3-0088-46ec-b052-ce1b3533b79d`): all 113 passed, none printed a runtime `SKIPPED`.

Reused: the workspace run above for everything outside those selections, since the fix changes one test's fixture, the
wording of one paragraph of the instructions Farhelm gives agents, the screenshot scene list (which feeds only the
screenshot comparison page, whose own checks ran), a GPUI parity notes page outside the docs site and a release-notes
draft, so the reference pictures and the docs site build were reused too; and #4674 changes only the opt-in real-agent
tests and documentation. Skipped: the desktop app's outer program's tests, which nothing in either plan reaches; macOS,
where nothing compiles differently; real agent programs and a real Tensorlake account; CI, which runs only on demand.

#### Things you should know that the report does not say

- If a new Tensorlake API key is saved and the next refresh fails, Settings keeps showing the sandboxes found with the
  old key until a refresh succeeds. That is not new (the list kept its rows on failure before too), and Delete on such a
  row is still safe: the helm checks again that the sandbox is one Farhelm created and not on this helm, and Tensorlake
  refuses an ID it does not know, but a possible follow-up is to forget the remembered rows when the key changes.
