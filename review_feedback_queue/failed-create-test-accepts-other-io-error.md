# Failed-create replay can test an unrelated refusal

Reviewed commit: ea133b9941769c69dc4683aa485b265dd09ed70c

## TLDR

The replay fixture can accept an unrelated working-directory I/O error without testing the launch-spec write refusal it
claims to replay.

## Details

The first error assertion in `crates/farhelm-supervisor/src/service/core.rs:27791–27887` accepts Internal. An
unclassified error checking the working directory takes that same class at `:3889–3935` before the launch-spec write.
The later replay, fingerprint conflict and missing-session assertions can then still pass. Assert the original
launch-spec failure context or observe the intended write stage. This alternate error is source-supported and its
occurrence on a test runner is unverified.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `test-substrate/l7`.
