# Full Podman inspection can strand a container that Farhelm needs to remove

Reviewed commit: b23bbba44fa7bcf0d111696ceb744c54042e6eff

## TLDR

A container with enough otherwise valid metadata can fail Farhelm's ownership inspection and remain undeletable from
Farhelm.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/environment/podman.rs:260`.

podman.rs:260–303 requests default full JSON, then consumes ID, name, state and labels. podman-container-inspect(1),
Description/Examples, documents full JSON with Config.Env and a --format projection. podman-run(1), Environment and
--unsetenv, documents default inheritance of image and containers.conf variables. Farhelm's run flags at
podman.rs:486–501 disable implicit host HTTP-proxy inheritance without clearing arbitrary image variables. The common
capture ceiling is 64 KiB (provisioning/backend.rs:51, :1936–1945, :1967–2007). Full JSON can exceed this independently
of the fields needed for ownership, causing an error before rm at podman.rs:450–470. Leftover Delete (:778–811),
row-backed destroy (:1439–1480) and helper reconciliation (build.rs:638–682) all pass through the same full inspection.
SPEC_impl.md:6144–6148 and :6191–6195 require confirmed, ownership-checked cleanup. Project only the identity, state and
ownership fields while retaining bounded capture and fail-closed validation. podman/tests.rs at tip :149–154 fakes only
the consumed fields and does not exercise output capacity. The full inspection is also present in the parent
7ceb053a0c34908b8f081ec48d2f31fd1b711a1a:260–269, so the blocker predates the volume flag. Bucket other for cleanup
blocked on configuration-specific input. Confidence confirmed by source and documented interface conditional on a
response above 64 KiB. No image inspection or engine run was observed. Possible coverage none.

Independent check of the proposed exclusion. AMBIGUOUS via b23bbba4 and df619914:podman.rs:258–287 and :448–472,
provisioning/backend.rs:49–61. The removal flag is reached only after inspection succeeds, so it cannot enlarge that
preceding inspect response. The immediate parent and the capture path applying the 64 KiB constant to inspect are
missing, leaving the full pre-existing scope claim unverified.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `podman/l12`.
