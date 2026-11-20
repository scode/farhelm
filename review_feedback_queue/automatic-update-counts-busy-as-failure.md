# Manual work can consume an automatic update attempt

Reviewed commit: aa0c139dd77691606c57740702210491840f63f4

## TLDR

An outdated host can miss automatic updates until it reconnects or Farhelm reopens if a manual operation takes the host
first and leaves it outdated.

## Details

The scan reserves the incarnation before it awaits planning in
`crates/farhelm-helm/src/provisioning/automatic.rs:113–159`. A competing run can then win the busy claim, causing
`crates/farhelm-helm/src/provisioning/service.rs:1896–1905` and `:2405–2414` to return Busy. The scan saves that as a
failed attempt. The completion wake reaches a scan that is already suppressing the host. SPEC.md, Topology says busy
hosts are reconsidered when their work finishes. Clear the provisional reservation on a typed Busy result and preserve
suppression for actual failures. The ordering is supported by source and remains unmeasured at runtime.

Suggested bucket: high. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `automatic-updates/l4`.
