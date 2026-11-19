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
