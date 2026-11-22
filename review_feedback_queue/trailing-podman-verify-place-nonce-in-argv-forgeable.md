# Podman place verification passes its proof marker in plaintext argv where a hostile peer can echo it

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

When verifying a container host's machine and account over SSH, Farhelm passes the expected proof nonce in plaintext in
the remote command arguments. If a container host's SSH alias accidentally moves to a different, compromised host that
does not know the original machine ID, that host can echo the nonce from argv to spoof container absence during Delete
(forgetting a live container and its sessions) or accept a pending container create on the wrong host.

## Details

**highest.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman_probe.rs:42`.

`VERIFY_PLACE_AND_EXEC` (`crates/farhelm-helm/src/environment/podman_probe.rs:37-51,152-190,206-214`) passes the
per-invocation `nonce` in plaintext argv (`$2`) and accepts stdout starting with `format!("{nonce}\n")` as proof that
the guard matched. A different, compromised SSH endpoint reached after an accidental SSH alias move can read `$2` from
the remote command line and echo it without knowing the original host's `machine_id`, allowing it to spoof container
absence during Delete (so the helm forgets a still-running container and its sessions on the original host) or accept a
pending `Podman::create` recovery on the wrong host.

Reviewer metadata (g35-podman-identity-probe/c3): originally set aside as a candidate (`proposed_drop` citing
`SPEC_impl.md:6250-6253` and `SPEC.md:5411-5416`); confidence: confirmed; possible cover: `SPEC_impl.md:6250-6253` and
`SPEC.md:5411-5416`.

Independent drop check (Entry D9, cid `74817247-85e5-4fc8-b186-06445a60ff01`): DISAGREE via
`podman_probe.rs:37-51,152-190,206-214`, `SPEC.md:5324-5328,5411-5425,5439-5441`, and `SPEC_impl.md:6250-6253`.
`SPEC.md:5422-5424` explicitly states that the accidental-vs-deliberate rule never relaxes the remote-host boundary or
weakens container-host SSH move protection, and `SPEC_impl.md:6250-6253` does not accept plaintext argv marker forgery.
Fix direction: verify a response digest derived from `(nonce, machine_id, uid)` rather than echoing plaintext argv `$2`,
or explicitly narrow `SPEC_impl.md` to benign-host drift.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g35-podman-identity-probe/c3`.
