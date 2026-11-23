# Program names on Stop and Delete approval cards

## What this was about

When an agent asked to stop or delete a Bash or Zsh session, the approval card could say the action would end an agent.
The maintainer chose “shell” for those sessions and wanted Delete to say which processes the user would allow Farhelm to
stop.

Stop now names the program and terminal tabs in its heading, including in the waiting list. Delete names the scope of
consent for a session with nothing running, terminal tabs only, or both its main program and tabs. Its wording and
consent come from the same live session snapshot.

## Things you should know

- Bash and Zsh use “shell.” Agent sessions use “agent.” Plain commands and sessions whose program kind is unavailable
  use “main program.” A new client can read a card from an older helm, and an older client can read one from a new helm.
- If Stop has no cached description, Farhelm gives the supervisor two seconds to supply one. If that read fails or runs
  out of time, the card uses “main program” and the Stop request proceeds to its usual approval check.
- The supervisor enforces the original Delete consent at dispatch. For example, approval to close only the terminal tabs
  cannot also end a main program that has since restarted.

## Open questions and possible follow-ups

None from this plan.

## PRs

- [#4688](https://github.com/scode/farhelm/pull/4688/changes) Program wording on Stop and Delete approvals.

## Checks

- The focused proto, helm, helm-client and desktop approval selection passed all 284 tests at `7e6acfc8`
  (`01c5e0b2-b48d-45fb-8da1-2e49e5d5aee9`). The final rerun covers the optional Stop read deadline, late replies, Stop
  facts and the deliberate difference between the cached and live Delete snapshots.
- Existing Linux approval and agent Add host screenshots matched their references in both themes
  (`a626438d-aa04-4cd8-a021-3ae51637cc1f` at `5ea28739`). No reference updates were needed.
- `cargo fmt --all -- --check`, `dprint check` on the changed Markdown, `git diff --check`,
  `python -B scripts/check-test-sleeps.py` and `python3 releasing/check-changelog.py format` passed.
- The 10 focused checks at `5ea28739` (`d35fb34a-0944-4c52-b6e3-c2b4c86a84ea`) all passed. They covered the optional
  Stop read, expired and late replies, and the revised Stop and Delete assertions. The same revision passed strict
  `cargo clippy --all-targets -- -D warnings`.
- Draft head `8094a6d3` carries the tested patch on `1ea1227b`. The focused rerun covers the new deadline and its
  interaction with Stop. The initial 284-test run covers the other approval behavior. Main moved the permission-chip
  wording and the environment Stop and suspend action, away from the covered approval and transport paths. The selected
  screenshot fixtures and reference pictures are unchanged across the rebase.
- Executor validation targets the affected code. The full suite is pending at handoff for the landing monitor.

<details>
<summary>Recorded test commands</summary>

The commands ran through the test recorder. Each nextest run used four slots and zero retries.

```sh
cargo nextest run --locked --test-threads 4 --retries 0 -p farhelm-proto -p farhelm-helm -p farhelm-helm-client -p farhelm-gui --lib -E 'test(/^approvals::/) | (package(farhelm-helm) & test(/^agent_requests::/)) | (package(farhelm-helm-client) & test(/^collections::/))'
cargo nextest run --locked --test-threads 4 --retries 0 -p farhelm-helm -p farhelm-gui --lib -E 'test(/^client::tests::.*(listing|full_queue|writer_failure)/) | test(/agent_requests::tests::(a_stop_waits_for_approval|approval_cards_name_uncached|agent_delete_preserves|an_agent_delete_refusal)|approvals::model_tests::stop_and_delete_cards/)'
python3 scripts/check-gui-goldens.py --scene '*approval*' --scene host-add-dialog-agent
```

</details>

## Review

The independent source review passed after bounding Stop's optional read and tightening the Stop and Delete fixtures.
The transport test verifies that a reply arriving after the deadline leaves the same connection usable and the later
waiter receives its own result.

### Landing

#### What landed with it

The plan's one PR (#4688) and one landing fix (#4694) landed in a round with codex-recipe-credentials-at-launch,
explain-stale-launcher-choices and wayland-clipboard-file-paste, none of which touches approval cards. Since the plan's
base, main had gained only Settings keeping its list of leftover Tensorlake sandboxes when a refresh fails and a TODO.md
entry; the rebase met only TODO.md, where this plan's removal of its own entry was kept.

The landing's independent reviewer confirmed that Delete's wording and the consent it records come from the same live
snapshot of the session, that the two-second wait for Stop's description is properly bounded and a late answer does not
break the connection, and that older and newer desktop apps and helms read the new field correctly.

#### Fixes made while landing

- The landing's comparison of every reference picture failed on one scene the plan had not republished: the terminal's
  covered-link scene, which draws a Stop approval card for a shell session over a terminal. Its heading now reads "Stop
  a session's shell and close its terminal tabs", as the plan intends, where it read "agent". #4694 republishes that
  scene's light and dark pictures; a full render found no other change. The report's "No reference updates were needed"
  missed this scene.

#### Checks

This round landed codex-recipe-credentials-at-launch (how Codex gets a recipe endpoint, in the helm and the supervisor,
with the protocol version bumped), explain-stale-launcher-choices (the desktop launcher), approval-card-program-wording
(the helm's approvals and the desktop's approval cards) and wayland-clipboard-file-paste (the vendored GPUI Linux
crate's clipboard and the terminal's paste) together. Between them they reach every crate except the desktop app's outer
program, so the landing ran the whole workspace suite, the lints (the desktop app's too), the end-to-end smoke test,
every reference picture and the docs site build on the four plans stacked on main, then narrower runs after the landing
fixes. The worker runs were on a separate arm64 Linux test worker with a systemd user manager, rootless Podman and SSH
to itself; the rest ran locally on x86_64 Linux. The first worker attempt (recorder run
`ef6ed206-f59e-44b9-9a19-e73ac2138ffe`) ran out of disk before it compiled anything; the disk was full of this landing's
own earlier build output, which was cleared, and the worker's runners now clean up after themselves (#4701). None of the
runs used a real agent program, a real Tensorlake account or a Wayland file manager: the tests use stand-ins, as the
reports say.

On the four plans as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `2dd259d9-fdaa-4006-b3ad-bf8181a7df43`): 8435 of 8437 passed. The two failures are Tensorlake setup tests that
  codex-recipe-credentials-at-launch broke, which its landing fix (#4693) corrects; the failed run is kept. One test, a
  check that stopping a session's containers reaches every selected scope, printed a runtime `SKIPPED` because it found
  no usable systemd user manager that time; run again on its own (`cargo nextest run -p farhelm --test e2e` selecting
  only `lifecycle_stops_containers_in_the_selected_scopes_before_signalling`, run
  `1d248544-da0e-41c5-a68b-d4113c9881c1`) it ran fully and passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's two
  `cargo clippy -p farhelm-desktop --all-targets` runs (with and without `--features agent`), on the test worker:
  passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `3b7e19df-96bb-4ec7-854c-83031f92e389`): of
  its 50 steps, 49 passed and one was skipped as always on Linux (paste, which headless Mutter cannot give the app).
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `f5a29c37-701b-4967-b9cd-7517d44f437b`): every picture
  but two matched its reference; the two are the terminal's covered-link scene, light and dark, whose approval card
  approval-card-program-wording rewords, republished by its landing fix (#4694). A full `--update` (recorder run
  `9fe8cb65-be4a-456f-af51-d4310849b6d8`) found those two the only changes.
- `python3 -B scripts/test-gui-screenshot-batch.py` (the screenshot scene list's own checks, locally and not through the
  recorder, so it has no run id), `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`,
  `dprint check`, `python3 releasing/check-changelog.py format` and the docs site build (`bun run build` in `website/`),
  locally: passed.

After the landing fixes (#4693 and #4694):

- `cargo nextest run -p farhelm-helm -E 'test(/tensorlake_add::tests::|credential_delivery/)'`, on the test worker
  (recorder run `1974b03e-abe5-4a18-b7c7-9716db0f0872`): all 65 passed, the two corrected tests among them.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker,
  with #4693 on main and the other three plans on top: passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

Reused: the workspace run and the smoke test above for everything else, since #4693 changes only those two tests, a
comment and a release-notes draft, and #4694 only two reference pictures' manifest entries. Skipped: the tests of the
desktop app's outer program (the part that starts the window and the embedded helm), which nothing here reaches; macOS,
where nothing compiles differently and the Wayland change does not apply; real agent programs, a real Tensorlake account
and a real Wayland file manager; CI, which runs only on demand.

#### Things you should know that the report does not say

- The plan removed the SPEC.md sentence "Approval lets this path close tabs even when the session has tabs open.", which
  contrasted an agent's approved Stop with the desktop's own Stop. Behavior is unchanged and the surrounding text still
  says an approved Stop closes every terminal tab; restoring the sentence would be a one-line follow-up.
