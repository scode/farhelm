# Interactive maintenance can hold all provisioning slots

Reviewed commit: fc361c4ba6a00ebe69be51fc81fe07ae034a1959

## TLDR

If four host operations are waiting for user input, another accepted operation can sit idle until one of those prompts
is answered or cancelled.

## Details

The owned run takes one of four fleet permits at `crates/farhelm-helm/src/provisioning/service.rs:2467–2476` and holds
it across revalidation and attachment. Both phases can wait for input, including the separate session connection during
Attach at `:3368–3369` and `:3433–3448`. SPEC.md, Waiting between operations on one host, scopes operation waits to the
affected host. Separate the per-host claim from the fleet permit so a wait for input need not occupy an active
provisioning slot. The permit lifetime and conditional delay follow from source. Actual fleet wait time remains
unmeasured.

Suggested bucket: high. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `maintenance-operations/l8`.
