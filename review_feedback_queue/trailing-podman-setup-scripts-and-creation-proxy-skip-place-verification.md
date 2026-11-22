# Container setup scripts and creation proxy lookup skip place-identity verification over SSH

Reviewed commit: 86be6c582492cddb7413e5a176d84bb29eb76da1

## TLDR

If a container host's SSH configuration changes outside Farhelm while an image build, preparation, or new container
setup is already underway (including changing the SSH `User` to another account on the same machine, which passes SSH
host-key verification silently), Farhelm can send the original account's proxy settings — which may contain proxy
passwords — over SSH to the new account or host, or install another host's cached proxy settings into the container.

## Details

**highest.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman.rs:433-497`.

(1) ControlMaster and eager stdin route: OpenSSH `ControlPath` in `crates/farhelm-helm/src/ssh.rs:605-609` uses `%C`
(hashing local host, remote host, port, and user), so changing `User` (or `HostName`) in `~/.ssh/config` does NOT reuse
the original ControlMaster; OpenSSH opens a fresh SSH connection to the new account/host (and a same-machine `User` edit
passes host-key checks silently). In `crates/farhelm-helm/src/environment/podman.rs:448-471`,
`Podman::script_with_proxy` prepends `build::proxy_exports(proxy)` (which `podman.rs:431` and `podman_probe.rs:3`
explicitly note can carry proxy passwords) and sends it via `self.0.places.command_with_input(...)`.
`SystemPlaceCommands::execute` (`crates/farhelm-helm/src/environment/podman_probe.rs:324-339`) spawns `ssh` and
immediately writes `script.as_bytes()` to `stdin` concurrently on spawn, with NO `VERIFY_PLACE_AND_EXEC` check. Every
`self.script(...)` step during multi-minute builds
(`crates/farhelm-helm/src/environment/podman/build.rs:60, 82, 187, 199, 210, 221, 230` — e.g. between `packages`,
`claude_code`, `codex`, and `prepare`), `Podman::proxies` (`podman.rs:679, 709, 748`), and post-create setup
(`podman.rs:1440`) therefore streams the original account's cleartext proxy credentials over SSH to the moved
account/host before `podman exec <id>` can exit on a missing container ID. (`Podman::proxies` -> `backend.install_bytes`
at `podman.rs:725-730` runs `cleanup_orphaned_temporaries` / `remove_owned_file` at `provisioning/backend.rs:1172-1193`
before `remote_put` at line 1272, so its file upload window is narrower — between lines 1193 and 1272 — whereas
`script_with_proxy` streams credentials immediately on every step.) Note that `SPEC.md:5402` defines the Unix account as
the local security boundary and `SPEC_impl.md:6251` / `podman_probe.rs:39-41` treat a moved SSH route as a
confidentiality boundary even for `/etc/machine-id`. Caveat: on a benign account running stock `podman`, `podman exec`
exits on missing container ID without reading stdin, so actual credential disclosure requires the moved account/host to
capture SSH stdin (e.g. a compromised/untrusted account or forced command). (2) At
`crates/farhelm-helm/src/environment/podman.rs:1448-1452`, `Podman::create` calls
`self.0.places.creation_proxy(&self.0.store, current.place_host)` without verifying `place.identity`
(`podman_probe.rs:719-726`). (3) At `crates/farhelm-helm/src/environment/podman/build.rs:399-400`
(`prepare_images_owned`) and `crates/farhelm-helm/src/environment/podman.rs:441-443` (`Podman::script`), the code
re-fetches `let (_, _, ready) = self.place(place).await?;` instead of using the `ready` observation already verified by
`helper()` (`build.rs:365-379`). Because `PodmanPlaces::runtime` (`podman_probe.rs:643-663`) returns `probe.result`
regardless of `probe.admission_refusal`, a background `Refresh` that ran while the alias temporarily pointed elsewhere
caches the other endpoint's `ReadyPlace` (including its `proxy` map) with `admission_refusal: Some(CHANGED_PLACE)`; if
the alias then moves back before `build.rs:400`, `proxies()` installs the other endpoint's proxies into the original
host's preparer container and `commit()` (`build.rs:421`) commits them into the prepared image. (By contrast, the other
`self.route()` callers — `prepare_user_manager` at `build.rs:125`, `preparer.prepare` at `build.rs:401`, and
`backend.restart` at `podman.rs:1461` — target a 64-hex container ID without streaming secrets on first contact and fail
closed.) Fix direction: pass the admission-verified `ready.proxy` into `script` and `build.rs:400` instead of re-reading
`self.place(place)`; verify `place_identity` in `creation_proxy`; and for `command_with_input` / `proxies`, wait for a
verified place-identity nonce on stdout before writing secret-bearing stdin (or explicitly bound this in
`SPEC_impl.md`). Corroborated independently by `g36/l4`.

Reviewer metadata (g38-podman-retarget-build-tests/l1): suggested bucket: highest (could be in highest: established
route in shipped code transmits cleartext proxy credentials across the Unix-account security boundary on an external
`~/.ssh/config` `User` change without host-key warning; caveat below on remote stdin capture); confidence: confirmed;
possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/farhelm-helm/src/environment/podman/build.rs:187-237` (g36-podman-identity-store-build/l4):

Farhelm explicitly treats proxy values as credential-bearing secrets
(`crates/farhelm-helm/src/environment/podman_probe.rs:3`: 'proxy values may hold credentials';
`crates/farhelm-helm/src/environment/podman.rs:426-432`: 'The script itself may contain proxy credentials, so receive it
through stdin into a mode-0600 temporary'). SPEC.md ('Local authority and trust between hosts', line 5402) defines the
local security boundary as 'the Unix account on a particular host.'

1. Unverified stdin write: `Podman::script` / `Podman::script_with_proxy`
   (`crates/farhelm-helm/src/environment/podman.rs:433-497`, called throughout `build.rs:60, 187-237, 400` and
   `podman.rs:678, 709, 748, 1440`) takes `place: HostId` rather than `PlaceTarget` and invokes
   `PodmanPlaces::command_with_input` (`podman_probe.rs:749-759`) -> `SystemPlaceCommands::execute`
   (`podman_probe.rs:288-350`), which does NOT wrap the remote shell in `VERIFY_PLACE_AND_EXEC`. `execute` spawns `ssh`
   and immediately writes `input` (beginning with `build::proxy_exports(proxy)`) to SSH stdin concurrently with
   `capture_child` (`podman_probe.rs:324-338`), before `podman exec` fails with no such container. (By contrast,
   `Podman::proxies` -> `backend.install_bytes` at `podman.rs:725` / `provisioning/backend.rs:1172-1272` first runs
   `cleanup_orphaned_temporaries` inside `podman exec`, so it errors on the absent container before `remote_put` uploads
   the proxy file.)
2. ControlMaster does NOT shield against external `~/.ssh/config` edits (Turn-2 verification):
   `SystemPlaceCommands::process` (`podman_probe.rs:272-274`) lowers via `BaseRoute::command` ->
   `RemoteRoute::Ssh::command(..., ConnectionPolicy::Shared(SharedConnection::Supervisor), ...)` (`access.rs:88-97`) ->
   `ssh_base_args` (`ssh.rs:344-351, 377-436`). `SharedConnection::Supervisor.socket_name()` (`ssh.rs:184`) is `%C`,
   passed as `ControlPath=.../%C` with `ControlMaster=auto` (`ssh.rs:427`). OpenSSH expands `%C` as a hash of
   `%l%h%p%r%j` (including remote `User` `%r` and resolved `HostName` `%h`). Therefore, an external edit to
   `~/.ssh/config` changing `User` (to another Unix account on the same machine, which passes host-key checks silently)
   or `HostName` immediately produces a different ControlPath, bypasses any live ControlMaster to the original account,
   and opens a fresh SSH connection to the new account — even without waiting for ControlMaster exit or fallback.
   Caveats: trigger requires (1) proxy variables containing credentials on the container host and (2) an external SSH
   alias/target change occurring mid-build between `helper()` and a subsequent `script()`/`proxies()` call. Per
   `review_feedback_queue/AGENTS.md` ('Priority buckets'), an established route keeps `highest` however rare its
   trigger. Fix direction: add verified place-identity wrapping (`VERIFY_PLACE_AND_EXEC`) to `command_with_input` (e.g.,
   `identity: Option<&PodmanPlaceIdentity>` on `command_with_input`, or a `run_verified_with_input` method) and pass
   `PlaceTarget` into `Podman::script` / `script_with_proxy` / `proxies` so the remote shell verifies
   `(machine_id, uid)` before `exec` reads private stdin (note: to prevent stdin from leaking on mismatch before the
   shell rejects it, either avoid sending private bytes until the shell emits the verified nonce marker on stdout, or
   gate with an initial verified handshake on the same shell).

Reviewer metadata (g36-podman-identity-store-build/l4): suggested bucket: highest (COULD BE IN highest and SHOULD be
bucketed as highest: established route in shipped code leaks credential-bearing proxy URLs across the Unix-account
security boundary defined in SPEC.md:5402); confidence: confirmed; possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/farhelm-helm/src/environment/podman_probe.rs:749` (g37-podman-retarget-probe-store/f5):

Commit 86be6c582492cddb7413e5a176d84bb29eb76da1 added same-shell place verification (`run_verified` /
`VERIFY_PLACE_AND_EXEC`) to `PodmanPlaces::command` (`crates/farhelm-helm/src/environment/podman_probe.rs:730-745`),
explicitly using a per-invocation digest "so a moved SSH route does not see the old machine ID"
(`podman_probe.rs:39-41`). However, `PodmanPlaces::command_with_input` (`podman_probe.rs:749-759`, called from
`Podman::script_with_proxy` at `crates/farhelm-helm/src/environment/podman.rs:465`) takes no `PodmanPlaceIdentity` and
`SystemPlaceCommands::execute` (`podman_probe.rs:324-339`) writes `input` — which starts with `proxy_exports(proxy)`,
carrying proxy URLs that `podman_probe.rs:3` notes may contain credentials — onto the SSH child's stdin concurrently
with process execution. Because `crates/farhelm-helm/src/ssh.rs:426-435, 505-507` keys `ControlPath` on OpenSSH `%C`
(hashing resolved host, port, and user), changing `HostName`, `Port`, or `User` for the SSH alias in `~/.ssh/config`
bypasses the previous host's `ControlMaster` and immediately dials the new host/account, transmitting the old host's
proxy credentials on stdin before `podman exec` fails for the missing 64-char container ID. Similarly,
`PodmanPlaces::creation_proxy` (`podman_probe.rs:719-726`, called at `podman.rs:1451`) calls `read_proxy` without
verifying `place_identity`.

Fix direction: Verify `place_identity` for `creation_proxy` (via `run_verified` for
`busctl`/`systemctl --user show-environment`), and for `command_with_input` accept `Option<&PodmanPlaceIdentity>` and
wait to observe the `VERIFY_PLACE_AND_EXEC` proof line on stdout before writing sensitive stdin bytes to `child.stdin`.

Reviewer metadata (g37-podman-retarget-probe-store/f5): suggested bucket: highest (confirmed credential-leak route:
OpenSSH %C in ssh.rs:505-507 hashes resolved HostName/Port/User, so an external ~/.ssh/config edit opens a fresh SSH
connection to the new host/account instead of reusing an existing ControlMaster, and SystemPlaceCommands::execute
streams cached proxy_exports — which podman_probe.rs:3 notes may contain proxy credentials — immediately on stdin to
that new endpoint); confidence: confirmed; possible cover: none (corroborates g36/l4 and g38/l1).

Note: part 3 of `g38/l1` (re-reading a moved probe's cached `ReadyPlace.proxy` via `self.place(place)` in
`build.rs:399-400` and `podman.rs:441-443`) also interacts directly with
`trailing-podman-refresh-caches-wrong-machine-on-refusal.md` (`g37/f2`).

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g38-podman-retarget-build-tests/l1`,
`g36-podman-identity-store-build/l4`, `g37-podman-retarget-probe-store/f5`.
