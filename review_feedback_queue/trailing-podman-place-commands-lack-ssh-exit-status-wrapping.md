# Podman place commands over SSH rely on raw exit codes without status-tag wrapping (unverified premise)

Reviewed commit: 74fa1951bac92f5a78757100534498c458d86806

## TLDR

Unlike host setup and update commands, Podman checks and container lifecycle commands over SSH trust the SSH client's
exit code directly. If a Linux container host's SSH server loses command exit statuses and reports zero on failure,
Podman checks can falsely pass, failed recipe build or proxy steps can be committed silently, and container or image
deletion can never confirm absence.

## Details

**other.** Unverified after investigation. Source inspection. Main reference
`crates/farhelm-helm/src/environment/podman_probe.rs:258-350`.

`SystemBackend::remote_command` (`crates/farhelm-helm/src/provisioning/backend.rs:816-837`) wraps every remote
provisioning script with `farhelm-command-status-v1 <tag> <status>` (`SPEC_impl.md:7971-7976`) so a lossy SSH server
reporting exit 0 cannot disguise a failed remote command. By contrast, `SystemPlaceCommands::process` and `execute`
(`crates/farhelm-helm/src/environment/podman_probe.rs:258-350`) run
`sh -c CLEAN_PLACE_EXEC farhelm-place-command <argv>` over `BaseRoute::Ssh` (now including `SshAccessKind::Tailscale`)
and trust the raw SSH child exit status (`result.code`). While `PlaceCommands::run_verified` (`podman_probe.rs:155-190`)
protects place identity via its stdout nonce (`VERIFY_PLACE_AND_EXEC` only prints the nonce after SHA-256 verification),
post-verification Podman commands still depend directly on `result.code`: (1) `PodmanPlaces::probe_inner`
(`podman_probe.rs:1072-1098`) checks `systemd-run --user --scope ... true` solely by `result.code == Some(0)`
(`podman_probe.rs:229`), so a broken systemd user scope passes the probe; (2) `Podman::script_with_proxy`
(`environment/podman.rs:448-497`) checks `output.code != Some(0)`, so failed bootstrap, recipe build, or `self.proxies`
scripts (`environment/podman/build.rs:400`) succeed silently and can be committed into a prepared image; (3)
`Podman::exists` (`environment/podman.rs:283-299`) maps exit 0 to `Ok(true)` and exit 1 to `Ok(false)`, so on a lossy
SSH server returning 0 it always reports existing—causing `fresh_image` (`build.rs:614-615`) to falsely report deleted
images as present and causing `remove_container` (`podman.rs:523-526`) and `retire_images_unlocked` (`build.rs:881-884`)
to fail closed permanently when trying to confirm deletion; and (4) `Podman::require` (`podman.rs:266-280`) for
`pause`/`unpause`/`stop` silently treats failed commands as succeeded. Note `SPEC_impl.md:7971-7976` currently specifies
the status-tag wrapper only for Provisioning, not for `SystemPlaceCommands`. Fix direction: if lossy SSH servers on
Linux are in scope for container hosts, wrap `BaseRoute::Ssh` invocations in `SystemPlaceCommands` with the same tagged
command-status sentinel used by `SystemBackend::remote_command`.

Reviewer metadata (g45-tailscale-provisioning-podman/f8): suggested bucket: other (not highest: place-identity
verification in `run_verified` checks the stdout nonce before trusting exit status, and container/image removal checks
`!self.exists(...)` which fails closed when exit 0 is returned; no security boundary bypass or data loss); confidence:
likely (unverified premise: whether any supported Linux SSH server, such as Linux Tailscale SSH or an SSH proxy, loses
remote command exit statuses and reports 0; `backend.rs:1766` cites Tailscale SSH on macOS as a known example, and macOS
cannot host Podman containers per `SPEC.md:5283,5387`, though `SPEC_impl.md:7972` states generally that "Some SSH
servers report zero even for a failed command"); possible cover: none (cross-group follow-up on `g33/c5` from
`g33-remote-supervisor-exit-status`).

Additional source record at `72dcafbc581259b7411c16f24995dab9f25f0986`,
`crates/farhelm-helm/src/environment/podman_probe.rs:288-350` (g33-remote-supervisor-exit-status/c5):

Revising Turn-1 candidate `c5` to a finding after inspecting all callers in
`crates/farhelm-helm/src/environment/podman.rs` and `podman/build.rs` (cross-group interface with
`g45-tailscale-provisioning-podman`). `SystemPlaceCommands::execute`
(`crates/farhelm-helm/src/environment/podman_probe.rs:288-350`) runs remote Podman/place commands over `BaseRoute::Ssh`
(which includes `SshAccessKind::Tailscale` and arbitrary user SSH servers) through `capture_child` directly without
`command_status::Tag` wrapping or status recovery. On an SSH server that reports exit 0 after a failed remote command:
(1) `Podman::exists` (`podman.rs:283-299`) misreads `podman container/image exists` exit 1 (absence) as `Ok(true)`,
causing `remove_container` (`podman.rs:523-526`) to permanently fail cleanup
(`"Podman still has the container; its ownership record was retained"`), `inspect` (`podman.rs:308-317`) to fail JSON
parsing instead of returning `Ok(None)`, and `software` (`podman/build.rs:158`) to treat missing images as present; (2)
`Podman::script` (`podman.rs:465-497`) treats failed recipe build scripts (`BOOTSTRAP`, `ready`, `packages`,
`claude_code`, `codex`, `prepare` in `podman/build.rs:60,187-237`) as succeeded when `output.code == Some(0)`, allowing
`commit` to capture broken software layers; (3) `Podman::require` (`podman.rs:266-280`) and `SystemPlaceCommands::run`
(`podman_probe.rs:225-232`) treat failed remote commands as exit 0. By contrast, `PlaceCommands::run_verified`
(`podman_probe.rs:174-187`) is NOT bypassed because it requires the per-invocation stdout nonce before trusting the
command (only degrading the error from `CONTAINER_PLACE_UNVERIFIED` to `container_host_unanswered`), and
`prepare_user_manager`/`finish_prepared_install` run through `SystemBackend` which is already wrapped. Fix direction:
apply the same tagged `command_status` wrapper (or equivalent status recovery) at the `BaseRoute::Ssh` boundary in
`SystemPlaceCommands::execute`.

Reviewer metadata (g33-remote-supervisor-exit-status/c5): suggested bucket: other (could NOT be in highest;
`run_verified` at `podman_probe.rs:174-187` requires its per-invocation stdout nonce before trusting a container place
command, ownership checks parse JSON from `podman container inspect`, and `exists` failing open as `true` causes cleanup
refusal rather than forgetting/deleting unowned resources; also note Tailscale SSH's known macOS exit-status loss
affects macOS hosts, whereas Podman places are Linux-only, though any lossy Linux SSH server triggers this); confidence:
confirmed; possible cover: none (commit #4617 scoped SPEC.md:5871-5872 and SPEC_impl.md:7971-7976 to remote setup,
Update, and uninstall via `SystemBackend`; unlike `transport.rs` at SPEC_impl.md:2688-2689, no spec section explicitly
accepts lossy-SSH failure for Podman place commands).

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g45-tailscale-provisioning-podman/f8`,
`g33-remote-supervisor-exit-status/c5`.
