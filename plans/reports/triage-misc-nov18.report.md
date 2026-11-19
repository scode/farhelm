## What this was about

When a terminal viewer's output stream ended, queued keystrokes could reach the terminal after a replacement viewer
started typing. Settings could offer Delete under **Containers not on this helm** during a first launch. A confirmation
answered after launch could remove the new session and its container. The agent admission test fixture kept a process
group number past the lifetime of the group, so reuse by the operating system could direct cleanup signals to an
unrelated group.

The recorded outcomes fix those three issues. Two other outcomes document existing behavior for Wayland clipboard timing
and launch-error excerpts after Restart.

## Things you should know

- Farhelm cancels an ending terminal viewer's queued input before a replacement viewer can send input. Input already
  handed to the terminal keeps its place.
- **Containers not on this helm** now checks the helm's current launch state as well as cached sessions. An interrupted
  launch or failed cleanup can still leave a container on the list for explicit deletion.
- The admission test checks the hosted group leader's recorded start identity immediately before a group signal. A child
  spawned by the test itself keeps the group number reserved through the signal when the child is live at the check.
- On Wayland, a key can reach the terminal while Farhelm is still fetching earlier pasted text. If a restarted terminal
  grows before launch failure, its error excerpt can include recovered output from the same session.

## Open questions and possible follow-ups

There is no decision outstanding. The container listing still uses the session cache after the launch ends. If a
successful launch fails to publish its cache entry, Settings can still show that container under **Containers not on
this helm** and offer Delete. That remains a possible follow-up.

## Pull requests

These are in landing order.

- Cancel the old terminal viewer's queued input before replacement.
  [#4156](https://github.com/scode/farhelm/pull/4156/changes)
- Keep containers that are still launching out of the leftovers list.
  [#4159](https://github.com/scode/farhelm/pull/4159/changes)
- Check process-group ownership in the agent admission test fixture.
  [#4163](https://github.com/scode/farhelm/pull/4163/changes)
- Document how typed input can overtake a pending Wayland paste.
  [#4169](https://github.com/scode/farhelm/pull/4169/changes)
- Document when a restart error can quote recovered terminal history.
  [#4170](https://github.com/scode/farhelm/pull/4170/changes)

## Checks

All four final nextest selections ran on Linux without runtime skips. Each recorder's HEAD below is its launch snapshot.
The nextest runs started with the relevant source patch applied where it was still in the working copy.

- At `6bc3ef2f` plus the detach patch, recorder `71e2073f-9080-412f-9b66-c4a2a171a6c9` passed all 70 selected tests. The
  command used `cargo nextest run -p farhelm-supervisor --lib` with
  `test(/^grid_output::/) | test(/^service::grid_stream::/) | test(/^service::connection::/)`.
- At `917d3cf3` plus the final container patch, recorder `db6856de-2f00-418c-8e13-9bf3fe73c861` passed
  `cold_container_launch_success_establishes_real_task_and_session_premise` and
  `normal_launch_preserves_ambiguous_send_or_failed_cleanup_after_waiter_loss` under
  `cargo nextest run -p farhelm-helm --lib`. The second test checks the same container with a failed launch and a
  running launch through the listing route, using injected Podman command results.
- At `0a9b501d` plus the admission fixture patch, recorder `090522a7-634b-43ff-8127-ce9c4a440d6b` passed 22 tests,
  including fixture cancellation and timeout cleanup and live Bun runs. The command used
  `cargo nextest run -p farhelm-supervisor --lib` with
  `test(/service::core::tests::(omp_.*|claude_report_survives_the_former_claim_deadline)/)`.
- The initial container run `bb9d7328-993e-4cbb-9b35-454fda64ef08` at `917d3cf3` plus the earlier container patch passed
  the cold launch success test. Its provider filter used the wrong module name. The provider run
  `e2145d7a-910d-445d-bd46-94297149ee0f` started at `48f4641b` and passed all five intended tests under
  `cargo nextest run -p farhelm-helm --lib` with
  `test(/^environment::podman::build::tests::.*(leftover|incomplete_session_rows|recorded_container_as_an_orphan|listing)/)`.
- `cargo clippy --locked -p farhelm-supervisor -p farhelm-helm --all-targets -- -D warnings` passed on the pre-rebase
  stack at `b7a196ce`. Cargo formatting, the Rust test-sleep checker and the instruction checker also passed before
  rebase. The instruction checker reported an existing archive citation warning.
- Rebased the stack onto `6e583272e` with tip `9be0de00`. Main changed launcher and submenu code, old terminal wording
  and a retired release notice. These are outside the tested teardown, listing and process-ownership paths. Each code,
  spec and changelog patch matched its pre-rebase patch, and the ledger rebase retains the three outcomes archived by
  the separate update-survival plan. The runtime and Clippy evidence still applies. Repeated dprint checks for the specs
  and fragments, the changelog format check, and whitespace checks after rebase.
- Skipped runtime checks for the two spec-only changes and the full workspace suite. The selected tests cover the
  affected runtime behavior.

## Review gate

Each draft passed an independent code review and a cold read of its commit message. Review strengthened the container
route regression test, distinguished terminal hangup from the shell reporters' sleep bound, and clarified that Restart
can carry forward only the history it recovers.

### Landing

#### What landed with it

It landed in a round of nine plans: session-group-follow-ups, model-families, session-limit-notice,
reserved-resource-names, secrets-logs-and-screen, checkout-settings-stale-save, concealed-link-target, triage-misc-nov18
and triage-discards-nov18. Since its base, main gained the previous round's plans (submenus that open on hover, Clone
keeping invisible characters, the retired old-terminal announcement, quick switcher screenshot folders, the update
survival wording) and docs screenshot tooling. reserved-resource-names, in this round, records the Podman leftover rules
that this plan's container change fits. The reviews found no interaction, and the triage records came through stacking
nine plans that each edit them with nothing lost or duplicated.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The input-order fix for a
viewer that detaches (PR 01), the leftovers list change (PR 02) and the Wayland paste note (PR 04) are correct and
small.

#### Fixes made while landing

- **The triage archive was left unformatted.** This plan's archive entries lacked a blank line and ran past the line
  width, so `dprint check` failed on main as the plan would have left it. Each PR's entry is now formatted.
- **A sentence described the Restart screen upside down.** The Restart quote change (PR 05) wrote, in SPEC_impl.md and a
  code comment, that Restart starts "with a blank screen above any terminal history it recovered". The history sits
  above the blank screen, as the code and the triage entry say, and the next sentence (growing the terminal pulls
  history into view) only makes sense that way. Both are fixed, and two choppy sentences beside them are joined again.
- **A changelog fragment was garbled.** It read "Settings → Containers not on this helm leaves out containers…". It now
  says that the list of containers not on this helm no longer shows a container whose first session is still launching.

#### Things you should know that the report does not say

- **The test fixture change for the OMP reporter (PR 03) went beyond its outcome.** Besides the change the outcome asked
  for, it now kills a directly started test process's group only if the process has not exited yet. If it has, the group
  kill is skipped and helper processes can linger for up to 25 seconds. That only matters when a test fails, and it
  closes a different hazard (signalling a process group after a timeout check had already reaped its leader, when the id
  could belong to another process), but the report does not mention the trade. The fixture's comments also lost their
  explanation of the order; I left them, since only tests read it. Possible follow-up: restore that explanation.
- **The detach fix's changelog fragment (PR 01) is marked as a fix** for an input-ordering problem the triage found no
  way to trigger in a released build. Release-time curation may want to drop it.

#### Checks

This round landed nine plans together, so these checks cover all nine on one tree: the nine stacks one after another on
main. Between them the plans change the desktop app's GUI crates, the helm and its client, the shared protocol, the
supervisor, the desktop process, SPEC.md, SPEC_impl.md and the triage records. The landing ran the lints, the whole Rust
workspace suite, the desktop process's own tests, the app's end-to-end smoke and the reference screenshots (goldens).
Builds ran locally; the workspace suite and the smoke ran on boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, `python3 -B scripts/test-gui-e2e-smoke.py`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`, once without and once with `--features agent`: passed.
- `dprint check`: failed on `TRIAGE_OUTCOMES_ARCHIVE.md`, which triage-misc-nov18 wrote unformatted. Fixed.
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats:
  2744, 2653 and 2625 tests, 8022 in all (`1ffb9472-9f12-4890-b0d5-032dd87718d0`,
  `7c3ef63b-2e2b-4903-acff-22cbe9409601`, `7c539d75-d409-48c7-901b-391169215c11`). One failed: the macro
  compile-contract test, which builds a small crate offline from this machine's cargo cache, which a boat does not have,
  so it fails there every time; it passed locally (below). No runtime skips.
- `cargo nextest run -p farhelm-desktop`, locally: 176 passed (`59017e5e-7626-4dca-9c45-0b4972a89aca`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed, the new step that
  drags an unpinned session into a group included, and the clipboard paste step was skipped, as always on Linux
  (`b1164f8b-3d55-4cac-9fb1-a76e9a8da60c`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`da0dca91-646f-483a-a436-b10b43ffafe2`), the new groups
  pictures included.

On the fixed tree (comments, SPEC.md, SPEC_impl.md, changelog fragments, the triage archive's formatting, and one
notice's wording changed):

- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, `cargo clippy --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`: passed.
- The bell notices' tests in the GUI and the helm, locally: 26 passed (`80fb276d-1ba3-4b09-a84a-370aef2b9605`).
- `cargo nextest run -p farhelm-testtrace-macros`, locally: 2 passed, the compile-contract test included
  (`a8a9bf1f-6dc9-4e24-8831-e5d4b7f5dbc0`).

The boat suites, the smoke and the goldens were reused for the fixed tree: after them only comments, documents,
changelog fragments, formatting and one notice's wording changed, which the focused runs above cover.

Skipped:

- The Linux app lifecycle: startup and quit are untouched.
- The Mac-only checks: no Mac-specific code changed.
