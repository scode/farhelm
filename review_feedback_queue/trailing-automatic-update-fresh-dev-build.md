# Automatic Update can replace a development build

Reviewed commit: e3dc96fc7b1597c703c9fc24bd91dc4dd781e9da

## TLDR

Automatic Update can replace a development supervisor even after a fresh probe identifies it as a development build.

## Details

**high.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/automatic.rs:139`.

SPEC.md:179–192 excludes development peers and independently disables automatic Update as an initiating policy for
staged host payloads. automatic.rs:84–104,115–160 gates only the initiating helm's source and the captured host
snapshot; DirectoryPayloads rejects automatic use (payloads.rs:236–240). The durable user preference is re-read at
admission (service.rs:1909–1927). That closes the settings opt-out race, not the peer-build race. Admission compares
entry UUID and incarnation only. manager.rs:4071–4079,4277–4300,6029–6046 and 1628–1635 show a new skew refusal can keep
the same token because neither refusal has a client. After that, planning and confirmation probes can report the
development marker and still pass the shared manual guards when schema permits (service.rs:435–478,1663–1755,2826–2940).
A devbuild can carry out a user-initiated Update (SPEC_impl.md:8170–8183), but this finding is about an independent
release helm already scanning. It does not require its target to be changed by that same helm or claim that a devbuild
can update unattended. The fresh probe must otherwise pass existing ownership, identity, schema and target checks.
Existing snapshot eligibility assertions at automatic.rs:199–250 and the committed preference opt-out gate cover
distinct boundaries. Keep the exemption on the fresh observation for automatic runs while preserving manual recovery.
Bucket: high for overwriting a build under test. No established user-work loss. Confidence: source-supported route for a
compatible development peer once an older eligible observation has started planning. No frequency or update outcome
observed. Possible coverage: automatic-update-misses-development-builds (the named fix is snapshot eligibility only).

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `provision/p4`.
