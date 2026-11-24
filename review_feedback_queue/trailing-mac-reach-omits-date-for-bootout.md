# Mac inspection omits `date` required by bootout wait, and bootout calls `date` only after unloading the job

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

Remote Mac setup, Update, and uninstall are specified to refuse during inspection if any command their shell path
requires is missing from the host's PATH. When Mac Update and uninstall were changed to track the 75-second launchd
bootout deadline with `date +%s`, `date` was not added to the Mac inspection command list. On a Mac with a restricted
PATH lacking `date` (or a non-standard `date` that rejects `+%s`), inspection succeeds instead of refusing with `date`,
and Update or uninstall unloads the running supervisor before failing on the missing command.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/backend.rs:4080`.

`SPEC.md`, "Topology" (lines 221-222) states: "There is no macOS version floor: inspection refuses a Mac missing a
command the path needs, naming that command." `SPEC_impl.md`, "Provisioning" (lines 8079-8080) likewise requires that
Mac inspection verify the commands the native path uses and refuse with the missing command's name. `MACOS_REACH_SH` in
`crates/farhelm-helm/src/provisioning/backend.rs:4080` checks every other external command used by the Mac path
(`launchctl readlink sha256sum stat install chmod mkdir mv cat head tr rm rmdir find ls grep id uname env sh mount sleep`),
including base `/bin` tools such as `sleep`, `sh`, `rm`, and `mv`, but does not include `date`. Commit
`4b60fe6467c54c6770caddc2570bf6b0800a36e1` added bare `now=$(date +%s) || return 1` calls to `bootout_job_sh()` in
`crates/farhelm-helm/src/provisioning/macos.rs:343,349` (invoked by `start_script` with `refresh = true` for Update and
by `stop_script` for uninstall). Furthermore, line 342 runs `launchctl bootout "$job_target" || return 1` _before_ line
343's `now=$(date +%s) || return 1`, so if `date` is absent or fails at runtime, the running supervisor is already
booted out before the command fails. Verify by inspecting `MACOS_REACH_SH` at `backend.rs:4080` and `bootout_job_sh()`
at `macos.rs:341-357`, or by running `start_script(..., true)` / `stop_script(...)` in `ShellFixture` with a failing
`date` stub and observing that `launchctl bootout` executes before failure. Fix: add `date` to `MACOS_REACH_SH`'s
`for tool in ...` list in `backend.rs:4080` (and optionally verify `date +%s` works alongside `readlink -f`), and in
`bootout_job_sh()` (`macos.rs:341-344`) verify `date +%s` (or read `now` before/after) so a failing `date` fails before
`launchctl bootout` unloads the job.

Reviewer metadata (g01_service_stop_grace/f1): suggested bucket: other — On a standard macOS host `/bin/date` sits
alongside `/bin/sleep`, `/bin/rm`, and `/bin/sh` which inspection already checks, so this only manifests with a
stripped/custom PATH or non-standard `date`. There is no security or data-loss impact (sessions survive
`AbandonProcessGroup=true` on Update, and uninstall already requires all sessions to have ended).; confidence:
confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g01_service_stop_grace/f1`.
