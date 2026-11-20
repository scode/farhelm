# Automatic Update misses the development-build exemption

Reviewed commit: 076932457a71c98fdbc187347b6bef668c231df7

## TLDR

A released Farhelm can overwrite a remote development supervisor if it speaks an older protocol. An update already in
progress can also miss a development build installed before the final check.

## Details

The Connected branch checks the development marker at `crates/farhelm-helm/src/provisioning/automatic.rs:82–98`. The
lower-protocol branch already has the build and omits that check. The fresh planning and execution probes apply the
ordinary manual-update policy in `crates/farhelm-helm/src/provisioning/service.rs:1653–1728` and `:2780–2868`. They can
therefore miss the exemption even when they observe a development build. This conflicts with SPEC.md, Topology. Apply
the same automatic eligibility rule to both reported connection states and to fresh version information before
scheduling or writing. The supported schema, payload, and ordinary target checks still apply. The gap is
source-confirmed. Runtime occurrence is unmeasured.

Suggested bucket: high. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `automatic-updates/l2`, `automatic-updates/l5`,
`transport-retirement/l8`.
