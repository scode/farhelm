# container_host_unanswered embeds up to 64 KiB of raw unsanitized remote stderr

Reviewed commit: c6e79ceeb43baace8a4f00345fd47c0fb6e1ec79

## TLDR

When a container host cannot be reached or fails before the host identity check runs, Farhelm copies up to 64 KiB of raw
SSH/remote error text directly into container status errors and operation failure messages without trimming length or
stripping control characters.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman_probe.rs:58`.

In `crates/farhelm-helm/src/environment/podman_probe.rs:58-66`, `container_host_unanswered` interpolates
`result.stderr.trim()` verbatim into the returned error string. `result.stderr` is captured by `capture_child`
(`crates/farhelm-helm/src/provisioning/backend.rs:52, 2079-2130`) up to `MAX_CHILD_STREAM_BYTES` (64 KiB) with no
sanitization. Every other Podman command stderr path in `crates/farhelm-helm/src/environment/podman.rs` (`require` L277,
`exists` L296, `scoped` L374, `run` L573, `commit` L652, `script_failure_output` L1760) runs `&output.stderr` through
`crate::tensorlake::shell::shown(...)` (`crates/farhelm-helm/src/tensorlake/shell.rs:194-213`), which strips
`farhelm_proto::text::is_presentation_unsafe` characters and bounds output to the last 12 non-empty lines and at most
2,000 characters. By contrast, `run_verified` (`podman_probe.rs:186`) returns `container_host_unanswered(&result)` raw.
Auditing all `PlaceTarget` callers with `identity: Some(...)` in `podman.rs` and `podman/build.rs` shows it flows
without further escaping or bounding into: (1) `Podman::status` (`podman.rs:1496, 1520-1522`) -> `StatusUnanswered` ->
`EnvironmentGate::Refuse` (`manager.rs:3921, 4959`) -> `HostState::Unreachable { last_error }` (`manager.rs:4406`),
serialized into `GET /api/hosts` (`hosts.rs:84-94`) and pushed over the UI channel for every container on that host
(note that `manager.rs:4946` wraps `DialAnyway` unanswered reasons in `peer_text`, whereas `manager.rs:4959` does not
wrap `Refuse`); (2) `Podman::destroy` (`podman.rs:1550-1581`), `Podman::change` (Suspend/Resume, `podman.rs:777-814`),
`Podman::create` (`podman.rs:1301-1369`), and `Podman::ensure_prepared` -> `fresh_image` (`podman.rs:1221`,
`podman/build.rs:610-615`), where it reaches HTTP error responses (`http_error`), launch failures,
`HostState::Deleting`/`Suspended` error text, and helm logs. (Turn-2 correction: `Podman::delete_leftover` at
`podman.rs:834, 864` and `Podman::list_owned`/`leftovers` at `podman.rs:1591, 1610` construct
`PlaceTarget::from(HostId)` with `identity: None`, so they take `run_bounded` rather than `run_verified`, and
`prepare_images`/`clean_images` do wrap errors through `shell::shown`.) Fix direction: pass `&result.stderr` through
`crate::tensorlake::shell::shown` in `container_host_unanswered` (and consider also bounding/escaping
`EnvironmentGate::Refuse`). Verify by adding a test in `podman_probe.rs` asserting that `run_verified` strips
control/Bidi characters and bounds multi-kilobyte stderr on pre-guard failure.

Reviewer metadata (g34-podman-recipe-rebuild/l4): suggested bucket: other (established impact is diagnostic/UX bloat up
to 64 KiB per container host status and unsanitized control characters in helm logs/HTTP error bodies; GUI escapes
`HostView.last_error` through `HostText`, so this would move to `highest` only if raw control/ANSI escape sequences from
an untrusted SSH peer reaching helm logs or a terminal client are treated as a security-impacting terminal-injection
route); confidence: confirmed; possible cover: none.

Additional source record at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`,
`crates/farhelm-helm/src/environment/podman_probe.rs:58` (g37-podman-retarget-probe-store/f7):

`container_host_unanswered` (`crates/farhelm-helm/src/environment/podman_probe.rs:58-66`), called by
`PlaceCommands::run_verified` (`podman_probe.rs:186`) whenever a command exits without emitting the verification marker
(and not guard exit 125 with empty stdout), formats `result.stderr.trim()` verbatim without passing it through
`crate::tensorlake::shell::shown(&result.stderr)` (which strips `farhelm_proto::text::is_presentation_unsafe` characters
and bounds lines/characters, as every sibling Podman error formatter in
`crates/farhelm-helm/src/environment/podman.rs:277, 296, 374, 573, 652` does). Because `capture_child` caps stderr at 64
KiB (`MAX_CHILD_STREAM_BYTES`), up to 64 KiB of raw peer-controlled stderr (including control characters, ANSI escapes,
or Unicode bidi overrides) propagates directly into `Podman::status` -> `StatusUnanswered` (`podman.rs:1519-1523`) ->
`EnvironmentGate::Refuse` (`manager.rs:3917-3921`) -> `HostState::Unreachable`, as well as `Podman::destroy` ->
`HostState::Deleting { failure }` (`hosts.rs:171-173`, `sessions.rs:1125-1127`) and HTTP error responses.

Fix: Format `detail` with `crate::tensorlake::shell::shown(&result.stderr)` inside `container_host_unanswered`.

Reviewer metadata (g37-podman-retarget-probe-store/f7): suggested bucket: other (corroborates g34/l4; surface if
unsanitized bidi/control characters in HostState::Unreachable/Deleting error text cross a presentation-spoofing boundary
under SPEC_impl.md "Host-supplied text"); confidence: confirmed; possible cover: none (corroborates g34/l4).

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g34-podman-recipe-rebuild/l4`,
`g37-podman-retarget-probe-store/f7`.
