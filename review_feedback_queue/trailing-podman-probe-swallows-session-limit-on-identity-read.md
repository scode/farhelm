# Podman probe swallows SSH session-limit refusal on the final identity read

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

If a container host hits its SSH session limit on the final identity-check step of a Podman probe, Farhelm reports that
it could not verify the host's machine and account and tells the user to check the SSH destination instead of naming the
SSH session limit.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman_probe.rs:1108`.

`SPEC_impl.md:6172-6173` states: "When the fallback login fails, launch refusal or connection unavailability names the
container host's SSH session limit." In `crates/farhelm-helm/src/environment/podman_probe.rs:994-1016`, `run()`
deliberately preserves `SessionLimitRefusal` errors rather than replacing them with the generic `reason` string (tested
by `probe_redacts_peer_errors_but_keeps_typed_session_limit_reason` at line 1219), and all seven earlier commands in
`probe_inner` (`podman_probe.rs:1024-1098`) propagate `SessionLimitRefusal` via `?`. However, the eighth command
(`PLACE_IDENTITY_READ`, lines 1101-1114) catches every error with `Err(_) => Err(IDENTITY_UNREADABLE)` (and
`ReadyPlace.identity` is `Result<PodmanPlaceIdentity, &'static str>` which cannot hold the dynamic `SessionLimitRefusal`
message). If SSH `MaxSessions` is exhausted during that final command, `fresh()` and `view()` fail with
`IDENTITY_UNREADABLE` ("Farhelm could not verify this container host's machine and account. Check its SSH destination
and press Refresh.") instead of the SSH session limit refusal. Fix direction: Check
`Err(error) if error.is::<crate::transport::SessionLimitRefusal>()` in `probe_inner` and either propagate it
(`return Err(error)`) or store `Result<PodmanPlaceIdentity, String>` in `ReadyPlace.identity` so `SessionLimitRefusal`'s
message reaches `admission_refusal`, `view()`, and `fresh()`.

Reviewer metadata (g35-podman-identity-probe/f1): suggested bucket: other (cannot be in highest: misreports the refusal
reason when SSH MaxSessions is hit on the last probe command, with no data loss or security impact).; confidence:
confirmed; possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/farhelm-helm/src/environment/podman_probe.rs:1101` (g37-podman-retarget-probe-store/f4):

`SPEC_impl.md:6172-6173` states: "When the fallback login fails, launch refusal or connection unavailability names the
container host's SSH session limit." Every earlier step of `probe_inner`
(`crates/farhelm-helm/src/environment/podman_probe.rs:1024-1098`) propagates `Err` from `run(...)`, which specifically
preserves `crate::transport::SessionLimitRefusal` (`podman_probe.rs:1009-1015`). However, at the final
`PLACE_IDENTITY_READ` command (`podman_probe.rs:1101-1114`), `match observed` maps every `Err(_)` (line 1113) to
`Err(IDENTITY_UNREADABLE)` ("Farhelm could not verify this container host's machine and account. Check its SSH
destination and press Refresh."). Because `ReadyPlace.identity` has type `Result<PodmanPlaceIdentity, &'static str>`, a
`SessionLimitRefusal` on this step is silently discarded and replaced by a misleading SSH-destination message. Fix:
either propagate `SessionLimitRefusal` (or allow `ReadyPlace.identity`'s error to carry a `String` / `Cow<'static, str>`
so runtime access for existing bindings is kept while launch refusal still names the host's SSH session limit).

Reviewer metadata (g37-podman-retarget-probe-store/f4): suggested bucket: other (not highest: misdiagnoses an SSH
MaxSessions refusal on the final probe command as an unreadable machine/account identity; existing bindings retain
runtime access and no data is lost); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g35-podman-identity-probe/f1`,
`g37-podman-retarget-probe-store/f4`.
