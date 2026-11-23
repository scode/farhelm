## What this was about

An SSH sign-in prompt from Setup, Update or Uninstall could leave **Remove** disabled in the desktop. A Remove sent by
another client or the final step of Uninstall could also stall behind a host or Podman check. The decision was to let
removal proceed and cancel the sign-in once the host is gone.

The desktop now offers **Remove** while a check is waiting for an SSH answer. On successful removal Farhelm closes the
question and ends its SSH process. A returning discovery reply or Setup confirmation stays bound to the original host
entry and address. Removing the host or changing its address means a stale check cannot recreate the entry or set up the
former destination.

## Things you should know

Once a provisioning run has started, you can remove the host after it finishes. If a Remove attempt is refused while its
sign-in check is finishing, the desktop keeps the sign-in result and continues the original check after the refusal
arrives. While a check is still waiting at an SSH prompt, Remove opens the existing confirmation.

## Open questions and possible follow-ups

None.

## Pull requests

- [#4714](https://github.com/scode/farhelm/pull/4714/changes) enables Remove during SSH sign-in and ties late Setup work
  to its original host entry. It resolves the matching TODO and review feedback.

## Checks

- Reused 212 passing helm tests from `af1a8a1e`. The subsequent change adjusted only the desktop test's click sequence.
  A rebuilt Farhelm payload, followed by `cargo nextest run --locked -p farhelm-helm --lib`, covered provisioning, SSH
  sign-in, Remove, Tailscale route locking and SSH discovery. The run used four slots with no retries and no runtime
  skips, including local and self-SSH provisioning (recorder `09b08915-e4c3-4149-b8ef-ea1e897b1056`). The two real cases
  also passed in isolation with the rebuilt payload (`a40df9e9-2e94-4f0a-be9d-eacbbedc7271`). The prior run
  (`803f2eb4-8762-49f8-b97e-fe31a12bd783`) had used a debug payload from before the protocol version bump and is
  retained as a failed run.
- Passed on `828777d1` on Linux. `cargo nextest run --locked -p farhelm-gui --lib` covered the Hosts, sign-in and
  operation-store tests (316 tests, four slots, no retries or runtime skips, recorder
  `13245459-a47a-4380-bcb0-f4e15fde031e`). The exact Remove test passed separately
  (`812fd08b-e154-4dc8-b1e2-aa13a1bb0419`) after correcting its fixture to dismiss the sign-in popover without
  cancelling the question. The earlier failure is retained under `63eb9383-e428-410e-b7e2-d751ff9003fa`.
- The recorded four-process-budget `check-gui-goldens.py --update --scene host-menu --scene host-remove-confirmation`
  run produced four Linux PNGs, each an exact match for its reference (`98b335b7-434d-4472-8a6a-5b46ad08217f`). No
  reference publication was needed. The targeted render covers the existing menu and confirmation in both themes, while
  the desktop regression exercises the pending sign-in state.
- Passed on `828777d1`. `cargo clippy --locked -j 4 --all-targets -- -D warnings`,
  `cargo clippy --locked -j 4 -p farhelm --bins -- -D warnings`, `cargo fmt --all -- --check`, `dprint check` on changed
  Markdown, `git diff --check`, `python -B scripts/check-test-sleeps.py` with the isolated interpreter,
  `python3 scripts/check-agents-md.py`, and `python3 releasing/check-changelog.py format`. The instruction checker
  reported one existing archived citation warning. The full runtime suite and docs-shot refresh were skipped because the
  affected helm and desktop paths have focused coverage and the screenshot layout is unchanged.

## Review

The review gate passed. The plan's Sol reviewer found that the desktop did not offer Remove during a pending sign-in and
found races around retargeting, reconciliation and replay. A fresh scope review confirmed the fixes fit the agreed
behavior. Sol verified the corrections, including preservation of a successful sign-in whose replay waits on a Remove
reply. Opus approved the menu and confirmation captures in both themes.

### Landing

#### What landed with it

The plan's PR (#4714) and one landing fix (#4718) landed alone in their round. Since its base, main had gained round
324's six plans (reduced motion, the remembered-tab fallback in the terminal relay, Muse conversation reporting, a
golden-pictures docs change, the repository cache redesign and the brick test's remote Mac uninstall). None of them
touches host provisioning, SSH sign-in, the host store or Remove, and the rebase met no conflicts. The reduced-motion
plan changed the desktop app's animation clock, which this plan's new desktop test advances; the landing's full
workspace run (under Checks) includes that test.

The landing's independent reviewer confirmed that cancelling a host's sign-in checks only after its row is deleted is in
the right order, that a probe or setup confirmation coming back for a removed or re-addressed host is refused without
breaking discovery of new destinations, that a stale confirmation cannot mark a replacement host busy, and that the
desktop lets only Remove through while that host's own sign-in is waiting.

#### Fixes made while landing

- The plan added a new kind of host probe request for registered SSH hosts (`ssh-host`): when the desktop runs setup
  again for such a host, it names that host's entry and the address it showed, so a setup resent after a sign-in is
  refused if the host was removed or given another address meanwhile. SPEC_impl.md listed the other kinds but not this
  one; #4718 adds it.

#### Checks

The plan ran only filtered helm and desktop library tests, before main gained round 324's changes, so the landing ran
the whole workspace suite, all lints, the end-to-end smoke (whose host menu steps cover Remove, Update and Uninstall)
and every reference picture on the plan stacked on main. The worker runs were on a separate arm64 Linux test worker with
a systemd user manager, rootless Podman and SSH to itself; the rest ran locally on x86_64 Linux. None of them signs in
to a real SSH host that asks a question; the tests use stand-ins, as the report says.

On the plan as delivered, stacked on main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `2a8e738b-1605-4adb-838e-4197363d1898`): all 8461 passed. One test printed a runtime `SKIPPED` (a tab lifecycle test
  that found no usable systemd user manager, which the plan does not touch); run alone on the worker afterwards
  (recorder run `8bdc57ac-d24e-40e4-96d6-bf9f9fa8d019`) it passed with the user manager and no `SKIPPED`, so the skip
  was most likely load on the worker during the full run.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's two
  `cargo clippy -p farhelm-desktop --all-targets` runs (with and without `--features agent`), on the test worker:
  passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `79e4a5cd-2efb-4e64-8676-d0e12fed3742`): of
  its 50 steps, 49 passed and one was skipped as always on Linux (paste, which headless Mutter cannot give the app).
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `452880bb-f234-4e48-8a7b-1d3207b3763d`): every picture
  matched its reference.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fix (#4718, one sentence in SPEC_impl.md): `dprint check`, locally, passed. Everything above is reused
for it, since it changes only that document. The workspace run includes the desktop window's own library (the Hosts
menu, sign-in and operation store tests among it). Skipped: the tests of the desktop app's outer program, a separate
crate this plan does not touch; macOS, since nothing in the change is platform-specific and the helm and desktop window
run the same code there; CI, which runs only on demand.

#### Things you should know that the report does not say

- The report's checks name `cargo nextest run --locked -p farhelm-helm --lib` and `-p farhelm-gui --lib`, which read as
  whole-library runs. Both were filtered to the provisioning, sign-in, Remove and Hosts tests (212 helm tests and 316 of
  the desktop window's tests, out of about 1,900 and 3,400). The landing's full workspace run, under Checks, covers the
  rest.
- Remove no longer waits for other background checks on the host to finish. Only Setup, Update and Uninstall sign-in
  checks are cancelled; a connection probe or a Podman check already running carries on after the host is gone. Their
  database writes are refused, so the removed host never comes back, but three small things can outlive the removal: a
  Tailscale key file for the removed address can be written again and left behind (it grants nothing, and key cleanup
  was already best effort), a shared SSH connection can be opened afterwards and linger for up to a minute, and one
  small internal entry per removal stays in memory until the helm restarts. Remove used to wait on the per-host SSH
  connection lock, and the TODO entry this plan resolved named guarding exactly these as that lock's job; Remove no
  longer takes it. A possible follow-up is to cancel those checks too.
- After a successful Remove, the desktop's waiting sign-in check still ends as a cancelled failure, or in a narrow race
  replays setup against the removed host, which the helm refuses. Neither shows anywhere, since the host's row is gone;
  ending the check quietly would be simpler.
- The plan grew past the one small PR it planned: besides the helm change, the desktop had to stop greying out Remove
  during a sign-in, without which the helm fix would not have reached anyone. It also added the `ssh-host` probe request
  described under the fixes. Most of what it guards was already covered by the plan's other checks on a probe's original
  entry; the case only it catches is a host removed and added again at the same address within one sign-in. A request of
  that kind naming a host that is not an SSH host is refused with a misleading "reconfigured since this attempt was
  made", reachable only by a hand-built request.
